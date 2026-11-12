pub(crate) fn open(config: Config, name: Name, socket: Socket) {
    let connection = rustls::ClientConnection::new(config, name);
    rustls::StreamOwned::new(connection, socket);
}
