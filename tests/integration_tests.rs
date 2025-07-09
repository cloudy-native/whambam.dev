// Copyright (c) 2025 Stephen Harrison
//
// Permission is hereby granted, free of charge, to any person obtaining a copy
// of this software and associated documentation files (the "Software"), to deal
// in the Software without restriction, including without limitation the rights
// to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
// copies of the Software, and to permit persons to whom the Software is
// furnished to do so, subject to the following conditions:
//
// The above copyright notice and this permission notice shall be included in all
// copies or substantial portions of the Software.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
// IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
// FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
// AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
// LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
// OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
// SOFTWARE.

use assert_cmd::prelude::*;
use predicates::str::contains;
use std::process::Command;
use test_utils::MockServer;

#[tokio::test]
async fn test_no_ui_option_integration() {
    let server = MockServer::start().await;

    let mut cmd = Command::cargo_bin("whambam").unwrap();
    cmd.arg(server.url())
        .arg("-n")
        .arg("10")
        .arg("-c")
        .arg("2")
        .arg("--no-ui");

    cmd.assert()
        .success()
        .stdout(contains("The --no-ui option is currently not supported."))
        .stdout(contains("The UI interface is required for this version."));
}
