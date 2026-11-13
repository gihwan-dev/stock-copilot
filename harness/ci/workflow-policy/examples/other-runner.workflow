{
  "name": "example",
  "on": {
    "push": {
      "branches": [
        "**"
      ]
    }
  },
  "permissions": {
    "contents": "read"
  },
  "jobs": {
    "build": {
      "runs-on": "self-hosted",
      "steps": [
        {
          "uses": "actions/checkout@3d3c42e5aac5ba805825da76410c181273ba90b1",
          "with": {
            "fetch-depth": 1,
            "persist-credentials": false
          }
        },
        {
          "uses": "taiki-e/install-action@cfdb7ed4ccc7c7dce37759eae89dc5674b469b39",
          "with": {
            "tool": "just@1.59.0,cargo-nextest@0.9.151,cargo-deny@0.20.2,cargo-llvm-cov@0.9.1"
          }
        },
        {
          "run": "just check",
          "env": {
            "BASE": "${{ github.sha }}"
          }
        }
      ]
    }
  }
}
