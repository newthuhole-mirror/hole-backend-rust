# hole-backend-rust


## 部署

### prepare database

```
sudo -u postgres psql
```

```postgresql
postgres=# CREATE USER hole WITH PASSWORD 'hole_pass';
CREATE ROLE
postgres=# CREATE DATABASE hole_v2 OWNER hole;
CREATE DATABASE
postgres=# \c hole_v2
You are now connected to database "hole_v2" as user "postgres".
hole_v2=# CREATE EXTENSION pg_trgm;
CREATE EXTENSION
hole_v2=# \q
```

```
./hole-thu --init-database
```
