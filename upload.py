import time
import struct
import stat
from typing import List
from pathlib import Path
import zipfile
from io import BytesIO
from base64 import b64encode
import os
import subprocess

from alibabacloud_tea_openapi.client import Client as OpenApiClient
from alibabacloud_tea_openapi import models as open_api_models
from darabonba.runtime import RuntimeOptions

aliyun_id = str(os.getenv("ALIYUN_ID"))
access_key_id = str(os.getenv("ALIYUN_ACCESS_KEY_ID"))
access_key_secret = str(os.getenv("ALIYUN_ACCESS_KEY_SECRET"))


def _make_extra_field(mtime: float) -> bytes:
    """生成 UT extra field (0x5455)，与 Linux zip 一致。"""
    # flag=0x01 表示只含 mtime
    return struct.pack("<HHB I", 0x5455, 5, 0x01, int(mtime))


def _add_dir(zf: zipfile.ZipFile, arcname: str, mtime: float):
    dirpath = arcname.replace("\\", "/")
    if not dirpath.endswith("/"):
        dirpath += "/"

    info = zipfile.ZipInfo(dirpath)
    info.create_system = 3
    info.create_version = 30
    info.extract_version = 10
    info.compress_type = zipfile.ZIP_STORED
    info.external_attr = (stat.S_IFDIR | 0o755) << 16
    info.date_time = time.localtime(mtime)[:6]
    info.extra = _make_extra_field(mtime)
    zf.writestr(info, b"")


def _add_file(zf: zipfile.ZipFile, file: Path, arcname: str):
    data = file.read_bytes()
    mtime = file.stat().st_mtime

    info = zipfile.ZipInfo(arcname.replace("\\", "/"))
    info.create_system = 3  # Unix
    info.create_version = 30  # 3.0
    info.extract_version = 10  # 1.0 (store 只需 1.0)
    info.compress_type = zipfile.ZIP_STORED
    info.external_attr = (stat.S_IFREG | 0o755) << 16  # 普通文件 + 755
    info.date_time = time.localtime(mtime)[:6]
    info.extra = _make_extra_field(mtime)
    zf.writestr(info, data)


def zip2base64(path_list: List[Path]) -> str:
    zip_buffer = BytesIO()
    added_dirs: set[str] = set()

    with zipfile.ZipFile(zip_buffer, "w", zipfile.ZIP_STORED) as zf:
        for path in path_list:
            if path.is_dir():
                for file in path.rglob("*"):
                    arcname = str(file.relative_to(path.parent))
                    if file.is_dir():
                        if arcname not in added_dirs:
                            added_dirs.add(arcname)
                            _add_dir(zf, arcname, file.stat().st_mtime)
                    elif file.is_file():
                        # 补齐父目录
                        parts = arcname.replace("\\", "/").split("/")
                        for i in range(1, len(parts)):
                            d = "/".join(parts[:i])
                            if d not in added_dirs:
                                added_dirs.add(d)
                                _add_dir(zf, d, file.parent.stat().st_mtime)
                        _add_file(zf, file, arcname)
            else:
                _add_file(zf, path, path.name)

    return b64encode(zip_buffer.getvalue()).decode()


def fc_upload(path_list: List[Path], region: str, function_name: str) -> None:
    config = open_api_models.Config(
        access_key_id=access_key_id,
        access_key_secret=access_key_secret,
        endpoint=f"{aliyun_id}.{region}.fc.aliyuncs.com",
    )

    open_api_client = OpenApiClient(config)

    runtime = RuntimeOptions()

    params = open_api_models.Params(
        action="UpdateFunction",
        version="2023-03-30",
        protocol="HTTPS",
        method="PUT",
        auth_type="AK",
        style="FC",
        pathname=f"/2023-03-30/functions/{function_name}",
        req_body_type="json",
        body_type="json",
    )
    request = open_api_models.OpenApiRequest(
        body={"code": {"zipFile": zip2base64(path_list)}}
    )

    open_api_client.call_api(params, request, runtime)


def main() -> None:
    subprocess.run([
        "cargo",
        "zigbuild",
        "--release",
        "--target",
        "x86_64-unknown-linux-musl",
    ])
    print("Build completed successfully.")

    fc_upload(
        [
            Path("target/x86_64-unknown-linux-musl/release/tanix_blog"),
            Path("public"),
        ],
        "cn-hangzhou",
        "blog",
    )
    print("Upload completed successfully.")


if __name__ == "__main__":
    main()
