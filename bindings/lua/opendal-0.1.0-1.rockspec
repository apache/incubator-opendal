package = "opendal"
version = "0.1.0-1"

source = {
    url = "git+https://github.com/apache/opendal/",
}

description = {
    summary = "Apache OpenDAL™ LUA binding: access data freely. ",
    detailed = [[
        OpenDAL is a data access layer that allows users to easily and efficiently retrieve data from various storage services in a unified way.
    ]],
    homepage = "https://opendal.apache.org/",
    license = " Apache-2.0"
}

dependencies = {
    "lua >= 5.1",
    "luarocks-build-rust-mlua = 0.2.0",
}

build = {
    type = "rust-mlua",
    modules = {
        ["opendal"] = "opendal_lua",
    },
    target_path = "target",
}
