# hole-backend-rust


## 部署

### prepare database

```sql
CREATE USER hole CREATEDB;
ALTER USER hole WITH PASSWORD "hole_pass";
```

```
$ diesel setup
```

```sql
\c hole_v2
CREATE EXTENSION pg_trgm;
```

```
$ diesel run
$ python3 tools/migdb.py
```
