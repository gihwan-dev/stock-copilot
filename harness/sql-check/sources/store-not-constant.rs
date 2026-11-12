pub fn remove(connection: &Connection, id: i64) -> Result<usize, Error> {
    connection.execute("DELETE FROM holdings WHERE id = ?1", [id])
}
