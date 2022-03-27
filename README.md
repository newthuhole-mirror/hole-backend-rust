# hole-backend-rust


## 部署

以下内容假设你使用 Ubuntu 20.04


目前只支持postgresql，对支持sqlite的追踪见 issue #1

安装postgresql (略)

安装redis-server (略)

### 准备数据库

进入:

```
sudo -u postgres psql
```

执行:

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
### 运行

#### 基于二进制文件

```
./hole-thu --init-database
./hole-thu
```

#### 基于源码

安装rust与cargo环境 (略)

clone 代码 (略)

```
cargo run --release -- --init-database
cargo run --release
```

或安装`diesel_cli`后

```
diesel migration run
cargo run --release
```
