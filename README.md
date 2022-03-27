# hole-backend-rust v1.0.0


## 部署

*以下内容假设你使用 Ubuntu 20.04*

目前只支持postgresql，对支持sqlite的追踪见 issue #1

安装postgresql (略)

安装redis (略)

### 准备数据库

进入:

```
sudo -u postgres psql
```

执行 (替换`'hole_pass'`为实际希望使用的密码):

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

创建 .env 文件，写入必要的环境变量。可参考 .env.sample。

#### 基于二进制文件

从[release](https://git.thu.monster/newthuhole/hole-backend-rust/releases)直接下载二进制文件

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

### 关于账号系统

+ 如果你希望使用自己的登录系统，将 `/_login/` 路径交由另外的后端处理，只需最终将用户名和token写入users表，并跳转到 `/?token=<token>`。

+ 如果你希望也使用闭社提供的授权来维护账号系统，使用 `https://thu.closed.social/api/v1/apps` 接口创建应用，并在.env或环境变量中填入client与secret。此操作不需要闭社账号。详情见[文档](https://docs.joinmastodon.org/client/token/#app)。
