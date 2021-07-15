Catnip
=======

[![Join us on Slack!](https://img.shields.io/badge/chat-on%20Slack-e01563.svg)](https://join.slack.com/t/demikernel/shared_invite/zt-t25ffjf9-2k7Y_594T8xn1GBWVYlQ2g)
[![Build](https://github.com/demikernel/catnip/actions/workflows/build.yml/badge.svg)](https://github.com/demikernel/catnip/actions/workflows/build.yml)
[![Test](https://github.com/demikernel/catnip/actions/workflows/test.yml/badge.svg)](https://github.com/demikernel/catnip/actions/workflows/test.yml)

_Catnip_ is a TCP/IP stack that focuses on being an embeddable, low-latency
solution for user-space networking.

Building and Running
---------------------

**1. Clone This Repository**
```
export WORKDIR=$HOME                                  # Change this to whatever you want.
cd $WORKDIR                                           # Switch to working directory.
git clone https://github.com/demikernel/catnip.git    # Clone.
```

**2. Install Prerequisites**
```
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh    # Get Rust toolchain.
```

**3. Build Catnip**
```
cd $WORKDIR/demikernel    # Switch to working directory.
cargo build               # Build catnip.
```

**4. Run Regression Tests**
```
cd $WORKDIR/demikernel           # Switch to working directory.
cargo test -- --test-threads 1   # Run with a single-thread.
```

Code of Conduct
---------------

This project has adopted the [Microsoft Open Source Code of Conduct](https://opensource.microsoft.com/codeofconduct/).
For more information see the [Code of Conduct FAQ](https://opensource.microsoft.com/codeofconduct/faq/)
or contact [opencode@microsoft.com](mailto:opencode@microsoft.com) with any additional questions or comments.


Usage Statement
--------------

This project is a prototype. As such, we provide no guarantees that it will
work and you are assuming any risks with using the code. We welcome comments
and feedback. Please send any questions or comments to one of the following
maintainers of the project:

- [Irene Zhang](https://github.com/iyzhang) - [irene.zhang@microsoft.com](mailto:irene.zhang@microsoft.com)
- [Pedro Henrique Penna](https://github.com/ppenna) - [ppenna@microsoft.com](mailto:ppenna@microsoft.com).

> By sending feedback, you are consenting to your feedback being used in the
> further development of this project.
