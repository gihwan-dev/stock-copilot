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
      "uses": "./.github/workflows/other.yml"
    }
  }
}
