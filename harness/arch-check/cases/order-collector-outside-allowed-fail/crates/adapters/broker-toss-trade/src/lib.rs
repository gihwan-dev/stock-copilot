use domain::OpenOrderCollector as Collector;

pub fn forge(scope: Scope) {
    let _ = Collector::start(scope);
}
