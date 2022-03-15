import sqlite3
from datetime import datetime

def mig_post(db_old, db_new):
    c_old = db_old.cursor()
    c_new = db_new.cursor()
    rs = c_old.execute(
        'SELECT id, name_hash, content, cw, author_title, '
        'likenum, n_comments, timestamp, comment_timestamp, '
        'deleted, is_reported, hot_score, allow_search '
        'FROM post WHERE deleted = false'
    )

    for r in rs:
        r = list(r)
        r[3] = r[3] or ''  # cw
        r[4] = r[4] or ''  # author_title
        r[8] = r[8] or r[7]  # comment_timestamp
        r[7] = datetime.fromtimestamp(r[7])
        r[8] = datetime.fromtimestamp(r[8])
        r[10] = r[10] or False  # comment
        c_new.execute(
            'INSERT OR REPLACE INTO posts VALUES({})'.format(','.join(['?'] * 13)),
            r
        )
    db_new.commit()

    c_old.close()
    c_new.close()


if __name__ == '__main__':
    db_old = sqlite3.connect('hole.db')
    db_new = sqlite3.connect('hole_v2.db')

    mig_post(db_old, db_new)

