pub fn forge(scope: Scope) {
    synth! {
        let _ = domain::[<Open Order Collector>]::start(scope);
    }
}
