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


def _add_file(zip_file: zipfile.ZipFile, file: Path, arcname: str):
    zinfo = zipfile.ZipInfo(arcname.replace("\\", "/"))
    zinfo.external_attr = (stat.S_IFDIR | 0o755) << 16 | 0x10
    zinfo.create_system = 3  # Unix
    zip_file.writestr(zinfo, file.read_bytes())


def zip2base64(path_list: List[Path]) -> str:
    zip_buffer = BytesIO()
    with zipfile.ZipFile(zip_buffer, "w", zipfile.ZIP_DEFLATED) as zip_file:
        for path in path_list:
            if path.is_dir():
                for file in path.rglob("*"):
                    if file.is_file():
                        _add_file(zip_file, file, str(file.relative_to(path.parent)))
            else:
                _add_file(zip_file, path, path.name)

    zip_bytes = zip_buffer.getvalue()

    return b64encode(zip_bytes).decode()


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
