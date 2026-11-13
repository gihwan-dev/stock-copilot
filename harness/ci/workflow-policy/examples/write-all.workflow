{
  "name": "example",
  "on": {
    "push": {
      "branches": [
        "**"
      ]
    }
  },
  "permissions": "write-all",
  "jobs": {
    "build": {
      "runs-on": "ubuntu-24.04-arm",
      "timeout-minutes": 30,
      "steps": [
        {
          "name": "checkout",
          "uses": "actions/checkout@3d3c42e5aac5ba805825da76410c181273ba90b1",
          "with": {
            "fetch-depth": 1,
            "persist-credentials": false
          }
        },
        {
          "name": "tools",
          "uses": "taiki-e/install-action@cfdb7ed4ccc7c7dce37759eae89dc5674b469b39",
          "with": {
            "tool": "just@1.59.0,cargo-nextest@0.9.151,cargo-deny@0.20.2,cargo-llvm-cov@0.9.1"
          }
        },
        {
          "name": "check",
          "run": "just linux-check",
          "env": {
            "SHA": "${{ github.sha }}"
          }
        }
      ]
    }
  }
}
