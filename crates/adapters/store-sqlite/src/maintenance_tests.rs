use crate::sql::tests as sql;
use crate::tests::{Fixture, clock, key, runtime};
use crate::{Database, DatabasePath};

fn pragma(db: &Database, statement: &'static str) -> i64 {
    runtime()
        .block_on(db.read(move |connection| {
            connection.query_row(statement, [], |row| row.get::<_, i64>(0))
        }))
        .expect("pragma")
}

#[test]
fn fresh_databases_use_incremental_auto_vacuum_and_reclaim_freed_pages() {
    let fixture = Fixture::new();
    let path = DatabasePath::new(fixture.path());
    let rt = runtime();
    let db = Database::open(&path, &key(), &clock()).expect("open");
    assert_eq!(pragma(&db, sql::AUTO_VACUUM), 2);
    rt.block_on(db.write(|transaction| transaction.execute_batch(sql::FILL_FREELIST)))
        .expect("fill");
    assert!(pragma(&db, sql::FREELIST) > 0);
    rt.block_on(db.reclaim_space()).expect("reclaim");
    assert_eq!(pragma(&db, sql::FREELIST), 0);
    rt.block_on(db.close()).expect("close");
    let reopened = Database::open(&path, &key(), &clock()).expect("reopen");
    assert_eq!(pragma(&reopened, sql::AUTO_VACUUM), 2);
    rt.block_on(reopened.reclaim_space())
        .expect("reclaim again");
    rt.block_on(reopened.close()).expect("close");
}
