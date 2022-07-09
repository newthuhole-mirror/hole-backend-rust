# hole-backend-rust v1.2.0


## 部署

### 使用docker

+ 安装docker-compose

+ 执行
```shell
mkdir hole
cd hole

# 下载docker-compose.yml
wget https://git.thu.monster/newthuhole/hole-backend-rust/raw/branch/master/docker-compose.yml

# 下载add_pg_trgm.sh
mkdir psql-docker-init
wget https://git.thu.monster/newthuhole/hole-backend-rust/raw/branch/master/psql-docker-init/add_pg_trgm.sh -O psql-docker-init/add_pg_trgm.sh 


# 下载镜像
docker-compose pull
# 初始化postgres
docker-compose up -d postgres

# 建表
docker-compose run --rm hole-thu hole-thu --init-database  

# 全部跑起来
docker-compose up -d
```

现在树洞后端应该已经运行在8000端口了

停止运行：

```shell
docker-compose stop
```

需要修改`docker-compose.yml`的情况：

+ 编辑services.hole-thu.environmen填入你的后端地址(用于登陆时的回调跳转)和前端地址(用于允许跨域)

+ 如果希望使用其他端口而非8000，编辑services.hole-thu.ports

+ 你可能需要映射postgres的5432端口用于在宿主机上连接以创建管理员账号，但也可以在`add_pg_trgm.sh`中添加创建管理员账号的sql语句或新增加一个`create_admin.sh`

+ 如果需要使用闭社登陆，请在services.hole-thu.environment中添加需要用到的更多环境变量(参考`.env.sample`)

### 使用源码编译

*以下内容假设你使用 Ubuntu 20.04*

安装rust与cargo环境 (略)

clone 代码 (略)

安装postgresql (略)

安装redis (略)

#### 准备数据库

进入:

```shell
sudo -u postgres psql
```

执行 (替换`hole_pass`为实际希望使用的密码):

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
#### 编译&运行

创建 .env 文件，写入必要的环境变量。可参考 .env.sample。

```shell
cargo run --release -- --init-database
cargo run --release
```

或安装`diesel_cli`后

```shell
diesel migration run
cargo run --release
```

### 基于二进制文件

安装与准备数据库同

从[release](https://git.thu.monster/newthuhole/hole-backend-rust/releases)直接下载二进制文件

```shell
./hole-thu --init-database
./hole-thu
```


## 关于账号系统

+ 如果你希望使用自己的登录系统，在Nginx或Apache中将 `/_login/` 路径交由另外的后端处理，只需最终将用户名和token写入users表，并跳转到 `/###token=<token>`。

+ 如果你希望也使用闭社提供的授权来维护账号系统，使用 `https://thu.closed.social/api/v1/apps` 接口创建应用，并在.env或环境变量中填入client与secret。此操作不需要闭社账号。详情见[文档](https://docs.joinmastodon.org/client/token/#app)。编译运行时，增加`--features mastlogin`: `cargo run  --release --features mastlogin`
