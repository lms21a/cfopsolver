use cfop_solver::{CrossMode, Cube, Efficiency, F2lStrategy, SolveConfig, SolveMethod, Solver};

fn main() -> cfop_solver::Result<()> {
    let cube = Cube::from_scramble("R U2 F' L D")?;
    let result = Solver::new(SolveConfig {
        method: SolveMethod::Cfop,
        cross_mode: CrossMode::XXCross,
        f2l_strategy: F2lStrategy::Lookahead,
        efficiency: Efficiency::Thorough,
        trace: true,
        exploration: true,
        max_exploration_states: 12,
        alternatives: 2,
        ..SolveConfig::default()
    })
    .solve(&cube)?;

    println!("state: {}", cube.facelets());
    println!("solution: {}", result.solution);
    for stage in result.stages {
        println!("{}: {}", stage.stage, stage.algorithm);
    }
    Ok(())
}
