# Trustfall Gitlab Adapter

This is an adapter for the Gitlab API. [Trustfall](https://github.com/obi1kenobi/trustfall/) query engine, can be used to query any data source or combination of data sources: databases, APIs, raw files (JSON, CSV, etc.), git version control, etc.

## Get Started

This code requires a Rust 1.59+ toolchain, which on UNIX-based operating systems can be installed
with `curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh`. For other operating systems,
follow the [official Rust instructions](https://www.rust-lang.org/tools/install).

If you already have a Rust toolchain, but it's a version older than 1.59, it's recommended to
upgrade it by running `rustup upgrade`.

Querying the GitLab API requires a personal access token, which is easy to get using your
GitLab account:

https://docs.gitlab.com/ee/user/profile/personal_access_tokens.html

This token should be stored in the `GITLAB_API_TOKEN` environment variable.

In addition the `GITLAB_HOST` environment variable should be set to the URL of your GitLab instance.

Once you've installed Rust and obtained a personal access token, execute the following code to download and compile the demo code:

```bash
git clone git@github.com:wseaton/trustfall-gitlab-adapter.git
cd trustfall-gitlab-adapter
cargo build --release
export GITLAB_API_TOKEN="< ... your GitLab token goes here ... >"
export GITLAB_HOST="< ... your Gitlab host goes here ... >"
```

You are now ready to run the demo code:
```bash
cargo run --release query contents-of-filtered-files.ron
```

## Debugging

### VSCode

#### Prerequisites

In VSCode you can install the [CodeLLDB](https://marketplace.visualstudio.com/items?itemName=vadimcn.vscode-lldb) along with [Rust Analyzer](https://marketplace.visualstudio.com/items?itemName=rust-lang.rust-analyzer). 

#### Environment Variables

Gitlab Environment Variables should be set in the `.vscode/settings.json` file and in the `.vscode/launch.json` file.

#### Debugging

Press `F5` to start debugging.

or

Open `src/main.rs` and click debug above the `main()` function. You can set breakpoints and step through the code.