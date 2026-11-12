pub(crate) struct Lookup {
    method: Kind,
    request: Kind,
}

pub(crate) fn list(client: &Client, url: &str, method: Kind) {
    let request = client.get(url);
    request.send();
    let delete = method;
    let execute = delete;
    let lookup = Lookup {
        method: execute,
        request: execute,
    };
    lookup.request;
}
