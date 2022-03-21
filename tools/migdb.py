import sqlite3
import psycopg2
from datetime import datetime

db_old = sqlite3.connect('hole.db')
# change hole_pass to your real password
db_new = psycopg2.connect("postgres://hole:hole_pass@localhost/hole_v2")
c_old = db_old.cursor()
c_new = db_new.cursor()

searchable = {}
post_d = {}

dt = datetime.now()


def mig_post():
    rs = c_old.execute(
        'SELECT id, name_hash, content, cw, author_title, '
        'likenum, n_comments, timestamp, comment_timestamp, '
        'deleted, is_reported, hot_score, allow_search '
        'FROM post ORDER BY id'
    )

    for r in rs:
        r = list(r)
        r[3] = r[3] or ''  # cw
        r[4] = r[4] or ''  # author_title
        r[8] = r[8] or r[7]  # comment_timestamp
        r[7] = datetime.fromtimestamp(r[7])
        r[8] = datetime.fromtimestamp(r[8])
        r[9] = bool(r[9])
        r[10] = bool(r[10] or False)  # comment
        r[12] = bool(r[12])
        searchable[r[0]] = r[12]
        r.insert(5, r[2].startswith('[tmp]\n'))
        # print(r)

        post_d[r[0]] = r[1:]

    max_id = r[0]
    for i in range(1, max_id + 1):
        r = post_d.get(i, [
            '', '', '', '', False, 0, 0, dt, dt, True, False, 0, False
        ])

        c_new.execute(
            (
                'INSERT INTO posts VALUES({}) '
                'ON CONFLICT (id) DO NOTHING'
            ).format(','.join(["DEFAULT"] + ['%s'] * 13)),
            r
        )

    db_new.commit()


def mig_user():
    rs = c_old.execute('SELECT name, token FROM user')

    for r in rs:
        # print(r)
        c_new.execute(
            'INSERT INTO users(name, token) VALUES(%s, %s) '
            'ON CONFLICT (name) DO NOTHING',
            r
        )
    db_new.commit()


def mig_comment():
    _start = 0
    _step = 1000
    while True:
        print("comment loop...", _start)
        rs = c_old.execute(
            'SELECT id, name_hash, author_title, content, timestamp, deleted, post_id '
            'FROM comment WHERE id > ? ORDER BY id LIMIT ?',
            (_start, _step)
        )
        r = None
        for r in rs:
            r = list(r)
            r[2] = r[2] or ''
            r[4] = datetime.fromtimestamp(r[4])
            r[5] = bool(r[5] or False)
            r.insert(6, searchable[r[6]])
            r.insert(3, r[3].startswith('[tmp]\n'))
            # print(r)
            c_new.execute(
                'INSERT INTO comments VALUES({})'.format(','.join(["DEFAULT"] + ['%s'] * 8)),
                r[1:]
            )
        if not r:
            break
        db_new.commit()

        _start = r[0]


if __name__ == '__main__':
    mig_user()
    mig_post()
    mig_comment()
    pass


c_old.close()
c_new.close()
