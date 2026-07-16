use std::path::PathBuf;

use pyo3::{
    exceptions::{PyRuntimeError, PyValueError},
    prelude::*,
    types::PyModule,
};

use crate::{
    generate_dataset, generate_to_jsonl, Cube, Difficulty, Error, GenerateConfig, SolveConfig,
    SolveMethod, Solver,
};

fn python_error(error: Error) -> PyErr {
    match error {
        Error::InvalidState(_) | Error::InvalidAlgorithm(_) => {
            PyValueError::new_err(error.to_string())
        }
        _ => PyRuntimeError::new_err(error.to_string()),
    }
}

fn json_object(py: Python<'_>, value: &impl serde::Serialize) -> PyResult<PyObject> {
    let encoded =
        serde_json::to_string(value).map_err(|error| PyRuntimeError::new_err(error.to_string()))?;
    Ok(PyModule::import(py, "json")?
        .call_method1("loads", (encoded,))?
        .into())
}

#[pyclass(name = "Cube")]
#[derive(Clone)]
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

    fn __str__(&self) -> String {
        self.inner.facelets()
    }

    fn __repr__(&self) -> String {
        format!("Cube('{}')", self.inner.facelets())
    }
}

#[pyclass(name = "Solver")]
struct PySolver {
    inner: Solver,
}

#[pymethods]
impl PySolver {
    #[new]
    #[pyo3(signature = (method="cfop", trace=false, exploration=false, max_exploration_states=72, alternatives=0, max_depth=30, probe_limit=1_000_000))]
    fn new(
        method: &str,
        trace: bool,
        exploration: bool,
        max_exploration_states: usize,
        alternatives: usize,
        max_depth: usize,
        probe_limit: u64,
    ) -> PyResult<Self> {
        let method = method.parse().map_err(python_error)?;
        Ok(Self {
            inner: Solver::new(SolveConfig {
                method,
                trace,
                exploration,
                max_exploration_states,
                alternatives,
                max_depth,
                probe_limit,
            }),
        })
    }

    fn solve(&self, py: Python<'_>, cube: &PyCube) -> PyResult<PyObject> {
        let result = py
            .allow_threads(|| self.inner.solve(&cube.inner))
            .map_err(python_error)?;
        json_object(py, &result)
    }

    fn solve_state(&self, py: Python<'_>, state: &str) -> PyResult<PyObject> {
        let cube = Cube::from_facelets(state).map_err(python_error)?;
        let result = py
            .allow_threads(|| self.inner.solve(&cube))
            .map_err(python_error)?;
        json_object(py, &result)
    }

    fn solve_scramble(&self, py: Python<'_>, scramble: &str) -> PyResult<PyObject> {
        let result = py
            .allow_threads(|| self.inner.solve_scramble(scramble))
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
#[allow(clippy::too_many_arguments)]
#[pyo3(signature = (count, seed=0, difficulty="hard", scramble_length=0, solutions_per_scramble=1, method="fast", threads=0, trace=false, exploration=false))]
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
) -> PyResult<PyObject> {
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
    )?;
    let records = py
        .allow_threads(|| generate_dataset(&config))
        .map_err(python_error)?;
    json_object(py, &records)
}

#[pyfunction]
#[allow(clippy::too_many_arguments)]
#[pyo3(signature = (path, count, seed=0, difficulty="hard", scramble_length=0, solutions_per_scramble=1, method="fast", threads=0, batch_size=1024, trace=false, exploration=false))]
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
    )?;
    py.allow_threads(|| generate_to_jsonl(&config, path, batch_size))
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
    Ok(GenerateConfig {
        count,
        seed,
        difficulty,
        scramble_length,
        solutions_per_scramble,
        method,
        trace,
        exploration,
        max_exploration_states: 72,
        threads,
    })
}

#[pymodule]
fn cfop_solver(_py: Python<'_>, module: &PyModule) -> PyResult<()> {
    module.add_class::<PyCube>()?;
    module.add_class::<PySolver>()?;
    module.add_function(wrap_pyfunction!(apply_moves, module)?)?;
    module.add_function(wrap_pyfunction!(verify_solution, module)?)?;
    module.add_function(wrap_pyfunction!(generate, module)?)?;
    module.add_function(wrap_pyfunction!(generate_jsonl, module)?)?;
    module.add("SOLVED_STATE", crate::SOLVED_FACELETS)?;
    module.add("STATE_FORMAT", crate::generator::STATE_FORMAT)?;
    module.add("__version__", env!("CARGO_PKG_VERSION"))?;
    Ok(())
}
