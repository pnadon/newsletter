# Newsletter

A working newsletter application which keeps track of subscribers, and sends them a confirmation email before adding them to the list.

The core functionality is mostly taken from [zero2prod](https://github.com/LukeMathWalker/zero-to-production) by Luca Palmieri, as an exercise on using Rust for production. It has been without a doubt one of the best resources for learning how to write *correct* and *reliable* software in Rust, for production purposes.

The main motivation behind this project was to both understand how web services work "under the hood" and how one would go about designing and implementing one. Subjects include but are not limited to:
- authentication
- parsing/validation
- logging/tracing
- error handling
- designing robust test suites
- ensuring correctness via type safety, functional design patterns, and Rust's ownership model.

Although much of the code is based off of `zero-to-production`, I have also made some changes here and there based on personal preference and as an effort to advance the project on my own. Examples include:
- Some personal decisions on error handling and parsing in certain parts
- Parsing / validating user-submitted content so that all errors are accumulated at once, instead of short-circuiting on the first error. This approach offers better insight to the user and saves them from submitting the form multiple times to uncover all of their mistakes.
- Instead of hard coding credentials I am using a combination of Vault (which I am hosting), Consul-Template, and DigitalOcean's App Spec.
