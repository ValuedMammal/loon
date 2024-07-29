DROP TABLE IF EXISTS account;
DROP TABLE IF EXISTS friend;

CREATE TABLE account (
    id INTEGER PRIMARY KEY,
    nick TEXT NOT NULL,
    descriptor BLOB NOT NULL
);

-- test 2-of-2 public descriptor
INSERT INTO account (nick, descriptor) values ("test", "wsh(multi(2,[7d94197e/84h/1h/0h]tpubDCmcN1ucMUfxxabEnLKHzUbjaxg8P4YR4V7mMsfhnsdRJquRyDTudrBmzZhrpV4Z4PH3MjKKFtBk6WkJbEWqL9Vc8E8v1tqFxtFXRY8zEjG/<0;1>/*,[9aa5b7ee/84h/1h/0h]tpubDCUB1aBPqtRaVXRpV6WT8RBKn6ZJhua9Uat8vvqfz2gD2zjSaGAasvKMsvcXHhCxrtv9T826vDpYRRhkU8DCRBxMd9Se3dzbScvcguWjcqF/<0;1>/*))");

CREATE TABLE friend (
    account_id INTEGER NOT NULL,
    quorum_id INTEGER NOT NULL,
    npub TEXT NOT NULL,
    alias TEXT,
    FOREIGN KEY (account_id) REFERENCES account(id),
    PRIMARY KEY (account_id, quorum_id)
);

-- valuedmammal
INSERT INTO friend (account_id, quorum_id, npub, alias) values (1, 0, "npub1r89krrt3u2cugr6aje5n8e0f9jtp6awqj7czhx0t5ga5v5x6gq6s90z95r", "mammal");

-- Chicken458
INSERT INTO friend (account_id, quorum_id, npub, alias) values (1, 1, "npub100au36unfamj5npttgyce9szdtd3a5vtrwnx7fsqmn4jdu5xnl0qhnm2jh", "chicken");
