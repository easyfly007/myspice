#[test]
fn solver_module_placeholder() {
    assert!(true);
}

#[test]
fn dense_solver_solves_simple_system() {
    use sim_core::solver::{DenseSolver, LinearSolver};

    let ap = vec![0, 2, 4];
    let ai = vec![0, 1, 0, 1];
    let ax = vec![3.0, 1.0, 1.0, 2.0];
    let mut rhs = vec![9.0, 8.0];

    let mut solver = DenseSolver::new(2);
    solver.prepare(2);
    solver.analyze(&ap, &ai).unwrap();
    solver.factor(&ap, &ai, &ax).unwrap();
    solver.solve(&mut rhs).unwrap();

    assert!((rhs[0] - 2.0).abs() < 1e-9);
    assert!((rhs[1] - 3.0).abs() < 1e-9);
}

#[cfg(feature = "klu")]
#[test]
fn klu_solver_solves_simple_system() {
    use sim_core::solver::{KluSolver, LinearSolver};

    let ap = vec![0, 2, 4];
    let ai = vec![0, 1, 0, 1];
    let ax = vec![3.0, 1.0, 1.0, 2.0];
    let mut rhs = vec![9.0, 8.0];

    let mut solver = KluSolver::new(2);
    solver.prepare(2);
    solver.analyze(&ap, &ai).unwrap();
    solver.factor(&ap, &ai, &ax).unwrap();
    solver.solve(&mut rhs).unwrap();

    assert!((rhs[0] - 2.0).abs() < 1e-9);
    assert!((rhs[1] - 3.0).abs() < 1e-9);
}
