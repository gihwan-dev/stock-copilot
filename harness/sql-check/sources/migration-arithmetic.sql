CREATE TABLE lots (
    id INTEGER PRIMARY KEY,
    quantity TEXT NOT NULL
) STRICT;

CREATE TRIGGER lots_double AFTER INSERT ON lots
BEGIN
    UPDATE lots SET quantity = NEW.quantity * 2 WHERE id = NEW.id;
END;
