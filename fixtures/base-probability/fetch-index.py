"""재생 실험에 사용한 공개 지수 일봉을 표준 출력으로 쓴다."""

import argparse
import hashlib
import json
import sys
import urllib.request

DIGESTS = {
    "KOSPI": "41f4a1cd33754ef469e83ecf7e1955158e3b6016d6b3e2abb4527257bc8258e6",
    "KOSDAQ": "86a0ca4a02bf2d77c95cd7936e1ac69884e6cd2fb353d429dccac231ecea14ca",
}
FIELDS = ("localDate", "openPrice", "highPrice", "lowPrice", "closePrice")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("index", choices=DIGESTS)
    index = parser.parse_args().index
    url = (
        f"https://api.stock.naver.com/chart/domestic/index/{index}/day"
        "?startDateTime=202101010000&endDateTime=202512312359"
    )
    with urllib.request.urlopen(url, timeout=30) as response:
        rows = json.load(response)
    text = "".join("\t".join(str(row[key]) for key in FIELDS) + "\n" for row in rows)
    data = text.encode("ascii")
    digest = hashlib.sha256(data).hexdigest()
    if digest != DIGESTS[index]:
        sys.exit(f"{index} 일봉이 최초 표본과 다릅니다. SHA-256 {digest}")
    sys.stdout.buffer.write(data)


if __name__ == "__main__":
    main()
