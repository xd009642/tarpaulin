---
name: Bug report
about: Create a report to help us improve
title: ''
labels: ''
assignees: xd009642

---

**Before raising the issue**

Here are some common things you can try:

* For accuracy of results `--engine llvm` to try the llvm coverage instrumentation if you're on Linux
* Linker issues `--no-dead-code` dead code linking can cause issues so disable it. This will significantly reduce the accuracy of results with ptrace so you should also use `--engine llvm` if it's not the default on your OS

Check the troubleshooting guide in the repo!

**Describe the bug**
A clear and concise description of what the bug is and if you're using the ptrace coverage engine (default on x64 Linux) or the llvm instrumentation.

**To Reproduce**
If possible provide a minimal  example or project where you've observed the issue. Describe how you called tarpaulin and if it seems helpful provide a copy of what tarpaulin printed out to you.

If using ptrace you can use `--dump-traces` to get a log file that can be used for diagnostics. For llvm coverage you can attach the profraw files in `target/tarpaulin/profraws`

**Expected behavior**
A clear and concise description of what you expected to happen.
