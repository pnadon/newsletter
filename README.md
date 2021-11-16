# Newsletter

A working newsletter application which keeps track of subscribers, and sends them a confirmation email before adding them to the list.

The core functionality is mostly taken from [zero2prod](https://github.com/LukeMathWalker/zero-to-production) by Luca Palmieri, as an exercise on using Rust for production.

I have also tweaked some parts of the code (personal preference etc.), and instead of hard coding credentials I am using a combination of Vault (which I am hosting), Consul-Template, and DigitalOcean's App Spec.
