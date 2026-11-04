`github-app-test.pem` is a disposable RSA key generated solely for the local
mock-GitHub tests in `github_app.rs`. It is not registered with GitHub and must
never be used for an installation. It lets tests exercise real RS256 signing
without external credentials or an OpenSSL executable at test time.
