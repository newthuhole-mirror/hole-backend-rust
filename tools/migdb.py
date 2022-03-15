import sqlite3
from datetime import datetime

db_old = sqlite3.connect('hole.db')
db_new = sqlite3.connect('hole_v2.db')
c_old = db_old.cursor()
c_new = db_new.cursor()


def mig_post():
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


def mig_user():
    rs = c_old.execute('SELECT name, token FROM user')

    for r in rs:
        c_new.execute(
            'INSERT OR REPLACE INTO users(name, token) VALUES(?, ?)',
            r
        )
    db_new.commit()


def mig_comment():
    rs = c_old.execute(
        'SELECT id, name_hash, author_title, content, timestamp, deleted, post_id '
        'FROM comment'
    )
    for r in rs:
        r = list(r)
        r[2] = r[2] or ''
        r[4] = datetime.fromtimestamp(r[4])
        r[5] = r[5] or False
        c_new.execute(
            'INSERT OR REPLACE INTO comments VALUES({})'.format(','.join(['?'] * 7)),
            r
        )
    db_new.commit()


if __name__ == '__main__':
    # mig_post()
    # mig_user()
    mig_comment()

c_old.close()
c_new.close()

