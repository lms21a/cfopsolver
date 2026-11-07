use std::path::PathBuf;

use pyo3::{
    exceptions::{PyRuntimeError, PyValueError},
    prelude::*,
    types::{PyAny, PyList, PyModule},
};
use serde::de::DeserializeOwned;

use crate::{
    generate_dataset, generate_to_jsonl, CrossMode, Cube, Difficulty, Efficiency, Error,
    F2lStrategy, FeatureSelection, GenerateConfig, SolveConfig, SolveMethod, Solver,
    TrainingVerifier, VerificationConfig,
};

fn python_error(error: Error) -> PyErr {
    match error {
        Error::InvalidState(_) | Error::InvalidAlgorithm(_) | Error::InvalidConfig(_) => {
            PyValueError::new_err(error.to_string())
        }
        _ => PyRuntimeError::new_err(error.to_string()),
    }
}

fn feature_selection(value: &str) -> PyResult<FeatureSelection> {
    value
        .parse()
        .map_err(|error: Error| PyValueError::new_err(error.to_string()))
}

fn json_object(py: Python<'_>, value: &impl serde::Serialize) -> PyResult<Py<PyAny>> {
    let encoded =
        serde_json::to_string(value).map_err(|error| PyRuntimeError::new_err(error.to_string()))?;
    Ok(py
        .import("json")?
        .call_method1("loads", (encoded,))?
        .unbind())
}

fn python_value<T: DeserializeOwned>(value: &Bound<'_, PyAny>) -> PyResult<T> {
    let py = value.py();
    let encoded: String = py
        .import("json")?
        .call_method1("dumps", (value,))?
        .extract()?;
    serde_json::from_str(&encoded)
        .map_err(|error| PyValueError::new_err(format!("invalid verifier input: {error}")))
}

#[pyclass(name = "Cube", module = "cfop_solver")]
struct PyCube {
    inner: Cube,
}

#[pymethods]
impl PyCube {
    #[new]
    #[pyo3(signature = (state=None))]
    fn new(state: Option<&str>) -> PyResult<Self> {
        let inner = match state {
            Some(value) => Cube::from_facelets(value).map_err(python_error)?,
            None => Cube::solved(),
        };
        Ok(Self { inner })
    }

    #[staticmethod]
    fn solved() -> Self {
        Self {
            inner: Cube::solved(),
        }
    }

    #[staticmethod]
    fn from_scramble(scramble: &str) -> PyResult<Self> {
        Ok(Self {
            inner: Cube::from_scramble(scramble).map_err(python_error)?,
        })
    }

    #[getter]
    fn state(&self) -> String {
        self.inner.facelets()
    }

    #[getter]
    fn is_solved(&self) -> bool {
        self.inner.is_solved()
    }

    fn apply(&mut self, moves: &str) -> PyResult<()> {
        self.inner.apply(moves).map_err(python_error)?;
        Ok(())
    }

    fn applied(&self, moves: &str) -> PyResult<Self> {
        Ok(Self {
            inner: self.inner.after(moves).map_err(python_error)?,
        })
    }

    fn verify(&self, solution: &str) -> PyResult<bool> {
        self.inner.verifies(solution).map_err(python_error)
    }

    fn serialize(&self) -> String {
        self.inner.facelets()
    }

    #[pyo3(signature = (selection="both"))]
    fn features(&self, py: Python<'_>, selection: &str) -> PyResult<Py<PyAny>> {
        let selection = feature_selection(selection)?;
        json_object(py, &self.inner.features(selection))
    }

    #[pyo3(signature = (encoding="cubie_ids"))]
    fn feature_vector(&self, py: Python<'_>, encoding: &str) -> PyResult<Py<PyList>> {
        let values =
            match encoding.to_ascii_lowercase().replace('-', "_").as_str() {
                "cubie" | "cubie_ids" => self.inner.cubie_feature_ids().to_vec(),
                "cubie_one_hot" => self.inner.cubie_one_hot().to_vec(),
                "facelet" | "facelet_ids" => self.inner.facelet_ids().to_vec(),
                "facelet_one_hot" => self.inner.facelet_one_hot().to_vec(),
                _ => return Err(PyValueError::new_err(
                    "encoding must be cubie_ids, cubie_one_hot, facelet_ids, or facelet_one_hot",
                )),
            };
        // PyO3 converts Vec<u8> into bytes. Preserve the public list[int] API.
        Ok(PyList::new(py, values)?.unbind())
    }

    fn __str__(&self) -> String {
        self.inner.facelets()
    }

    fn __repr__(&self) -> String {
        format!("Cube('{}')", self.inner.facelets())
    }
}

#[pyclass(name = "Solver", module = "cfop_solver")]
struct PySolver {
    inner: Solver,
}

#[pyclass(name = "TrainingVerifier", module = "cfop_solver")]
struct PyTrainingVerifier {
    inner: TrainingVerifier,
}

#[pymethods]
impl PyTrainingVerifier {
    #[new]
    #[pyo3(signature = (threads=0, features="none"))]
    fn new(threads: usize, features: &str) -> PyResult<Self> {
        let features = feature_selection(features)?;
        Ok(Self {
            inner: TrainingVerifier::new(VerificationConfig { threads, features })
                .map_err(python_error)?,
        })
    }

    fn verify(&self, py: Python<'_>, item: &Bound<'_, PyAny>) -> PyResult<Py<PyAny>> {
        let input: serde_json::Value = python_value(item)?;
        let result = py.detach(|| self.inner.verify_json(&input));
        json_object(py, &result)
    }

    fn verify_batch(&self, py: Python<'_>, items: &Bound<'_, PyAny>) -> PyResult<Py<PyAny>> {
        let inputs: Vec<serde_json::Value> = python_value(items)?;
        let result = py.detach(|| self.inner.verify_json_batch(&inputs));
        json_object(py, &result)
    }
}

#[pymethods]
impl PySolver {
    #[new]
    #[pyo3(signature = (method="cfop", trace=false, exploration=false, max_exploration_states=72, alternatives=0, max_depth=30, probe_limit=1_000_000, cross="cross", f2l="lookahead", efficiency="balanced", decisions=false, max_decision_records=512))]
    #[allow(clippy::too_many_arguments)]
    fn new(
        method: &str,
        trace: bool,
        exploration: bool,
        max_exploration_states: usize,
        alternatives: usize,
        max_depth: usize,
        probe_limit: u64,
        cross: &str,
        f2l: &str,
        efficiency: &str,
        decisions: bool,
        max_decision_records: usize,
    ) -> PyResult<Self> {
        let method = method.parse().map_err(python_error)?;
        let cross_mode: CrossMode = cross.parse().map_err(python_error)?;
        let f2l_strategy: F2lStrategy = f2l.parse().map_err(python_error)?;
        let efficiency: Efficiency = efficiency.parse().map_err(python_error)?;
        let config = SolveConfig {
            method,
            cross_mode,
            f2l_strategy,
            efficiency,
            trace,
            exploration,
            max_exploration_states,
            decisions,
            max_decision_records,
            alternatives,
            max_depth,
            probe_limit,
        };
        config.validate().map_err(python_error)?;
        Ok(Self {
            inner: Solver::new(config),
        })
    }

    fn solve(&self, py: Python<'_>, cube: &PyCube) -> PyResult<Py<PyAny>> {
        let result = py
            .detach(|| self.inner.solve(&cube.inner))
            .map_err(python_error)?;
        json_object(py, &result)
    }

    fn solve_state(&self, py: Python<'_>, state: &str) -> PyResult<Py<PyAny>> {
        let cube = Cube::from_facelets(state).map_err(python_error)?;
        let result = py
            .detach(|| self.inner.solve(&cube))
            .map_err(python_error)?;
        json_object(py, &result)
    }

    fn solve_scramble(&self, py: Python<'_>, scramble: &str) -> PyResult<Py<PyAny>> {
        let result = py
            .detach(|| self.inner.solve_scramble(scramble))
            .map_err(python_error)?;
        json_object(py, &result)
    }
}

#[pyfunction]
fn apply_moves(state: &str, moves: &str) -> PyResult<String> {
    Cube::from_facelets(state)
        .and_then(|cube| cube.after(moves))
        .map(|cube| cube.facelets())
        .map_err(python_error)
}

#[pyfunction]
fn verify_solution(state: &str, solution: &str) -> PyResult<bool> {
    Cube::from_facelets(state)
        .and_then(|cube| cube.verifies(solution))
        .map_err(python_error)
}

#[pyfunction]
#[pyo3(signature = (state, selection="both"))]
fn cube_features(py: Python<'_>, state: &str, selection: &str) -> PyResult<Py<PyAny>> {
    let cube = Cube::from_facelets(state).map_err(python_error)?;
    let selection = feature_selection(selection)?;
    json_object(py, &cube.features(selection))
}

/// Verify a whole rollout batch in one Python-to-Rust call. Invalid cube or
/// move strings remain ordinary per-item results.
#[pyfunction]
#[pyo3(signature = (items, threads=0, features="none"))]
fn verify_batch(
    py: Python<'_>,
    items: &Bound<'_, PyAny>,
    threads: usize,
    features: &str,
) -> PyResult<Py<PyAny>> {
    let inputs: Vec<serde_json::Value> = python_value(items)?;
    let features = feature_selection(features)?;
    let verifier =
        TrainingVerifier::new(VerificationConfig { threads, features }).map_err(python_error)?;
    let result = py.detach(|| verifier.verify_json_batch(&inputs));
    json_object(py, &result)
}

#[pyfunction]
#[allow(clippy::too_many_arguments)]
#[pyo3(signature = (count, seed=0, difficulty="hard", scramble_length=0, solutions_per_scramble=1, method="fast", threads=0, trace=false, exploration=false, cross="cross", f2l="lookahead", efficiency="balanced", max_depth=30, probe_limit=1_000_000, decisions=false, max_decision_records=512, features="none"))]
fn generate(
    py: Python<'_>,
    count: usize,
    seed: u64,
    difficulty: &str,
    scramble_length: usize,
    solutions_per_scramble: usize,
    method: &str,
    threads: usize,
    trace: bool,
    exploration: bool,
    cross: &str,
    f2l: &str,
    efficiency: &str,
    max_depth: usize,
    probe_limit: u64,
    decisions: bool,
    max_decision_records: usize,
    features: &str,
) -> PyResult<Py<PyAny>> {
    let config = py_generate_config(
        count,
        seed,
        difficulty,
        scramble_length,
        solutions_per_scramble,
        method,
        threads,
        trace,
        exploration,
        cross,
        f2l,
        efficiency,
        max_depth,
        probe_limit,
        decisions,
        max_decision_records,
        features,
    )?;
    let records = py
        .detach(|| generate_dataset(&config))
        .map_err(python_error)?;
    json_object(py, &records)
}

#[pyfunction]
#[allow(clippy::too_many_arguments)]
#[pyo3(signature = (path, count, seed=0, difficulty="hard", scramble_length=0, solutions_per_scramble=1, method="fast", threads=0, batch_size=1024, trace=false, exploration=false, cross="cross", f2l="lookahead", efficiency="balanced", max_depth=30, probe_limit=1_000_000, decisions=false, max_decision_records=512, features="none"))]
fn generate_jsonl(
    py: Python<'_>,
    path: PathBuf,
    count: usize,
    seed: u64,
    difficulty: &str,
    scramble_length: usize,
    solutions_per_scramble: usize,
    method: &str,
    threads: usize,
    batch_size: usize,
    trace: bool,
    exploration: bool,
    cross: &str,
    f2l: &str,
    efficiency: &str,
    max_depth: usize,
    probe_limit: u64,
    decisions: bool,
    max_decision_records: usize,
    features: &str,
) -> PyResult<()> {
    let config = py_generate_config(
        count,
        seed,
        difficulty,
        scramble_length,
        solutions_per_scramble,
        method,
        threads,
        trace,
        exploration,
        cross,
        f2l,
        efficiency,
        max_depth,
        probe_limit,
        decisions,
        max_decision_records,
        features,
    )?;
    py.detach(|| generate_to_jsonl(&config, path, batch_size))
        .map_err(python_error)
}

#[allow(clippy::too_many_arguments)]
fn py_generate_config(
    count: usize,
    seed: u64,
    difficulty: &str,
    scramble_length: usize,
    solutions_per_scramble: usize,
    method: &str,
    threads: usize,
    trace: bool,
    exploration: bool,
    cross: &str,
    f2l: &str,
    efficiency: &str,
    max_depth: usize,
    probe_limit: u64,
    decisions: bool,
    max_decision_records: usize,
    features: &str,
) -> PyResult<GenerateConfig> {
    let difficulty = match difficulty.to_ascii_lowercase().as_str() {
        "easy" => Difficulty::Easy,
        "medium" => Difficulty::Medium,
        "hard" => Difficulty::Hard,
        _ => {
            return Err(PyValueError::new_err(
                "difficulty must be easy, medium, or hard",
            ))
        }
    };
    let method: SolveMethod = method.parse().map_err(python_error)?;
    let cross_mode: CrossMode = cross.parse().map_err(python_error)?;
    let f2l_strategy: F2lStrategy = f2l.parse().map_err(python_error)?;
    let efficiency: Efficiency = efficiency.parse().map_err(python_error)?;
    let features = feature_selection(features)?;
    Ok(GenerateConfig {
        count,
        seed,
        difficulty,
        scramble_length,
        solutions_per_scramble,
        method,
        cross_mode,
        f2l_strategy,
        efficiency,
        max_depth,
        probe_limit,
        trace,
        exploration,
        max_exploration_states: 72,
        decisions,
        max_decision_records,
        features,
        threads,
    })
}

#[pymodule]
fn cfop_solver(py: Python<'_>, module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<PyCube>()?;
    module.add_class::<PySolver>()?;
    module.add_class::<PyTrainingVerifier>()?;
    module.add_function(wrap_pyfunction!(apply_moves, module)?)?;
    module.add_function(wrap_pyfunction!(verify_solution, module)?)?;
    module.add_function(wrap_pyfunction!(cube_features, module)?)?;
    module.add_function(wrap_pyfunction!(verify_batch, module)?)?;
    module.add_function(wrap_pyfunction!(generate, module)?)?;
    module.add_function(wrap_pyfunction!(generate_jsonl, module)?)?;
    module.add("SOLVED_STATE", crate::SOLVED_FACELETS)?;
    module.add("STATE_FORMAT", crate::generator::STATE_FORMAT)?;
    module.add("DECISION_TRACE_SCHEMA", crate::DECISION_TRACE_SCHEMA)?;
    module.add("CUBE_FEATURE_SCHEMA", crate::CUBE_FEATURE_SCHEMA)?;
    module.add("CUBIE_FEATURE_FORMAT", crate::CUBIE_FEATURE_FORMAT)?;
    module.add("FACELET_ID_FORMAT", crate::FACELET_ID_FORMAT)?;
    module.add("CUBIE_FEATURE_WIDTH", crate::CUBIE_FEATURE_WIDTH)?;
    module.add("CUBIE_ONE_HOT_WIDTH", crate::CUBIE_ONE_HOT_WIDTH)?;
    module.add("FACELET_ID_WIDTH", crate::FACELET_ID_WIDTH)?;
    module.add("FACELET_ONE_HOT_WIDTH", crate::FACELET_ONE_HOT_WIDTH)?;
    module.add(
        "CUBIE_FEATURE_CARDINALITIES",
        PyList::new(py, crate::CUBIE_FEATURE_CARDINALITIES)?,
    )?;
    module.add("VERIFICATION_SCHEMA", crate::VERIFICATION_SCHEMA)?;
    module.add("__version__", env!("CARGO_PKG_VERSION"))?;
    Ok(())
}
