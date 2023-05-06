# A Rust implementation of the pro tanki / gtanks protocol
This is a simple implementation of the pro tanki / gtanks protocol written in rust.  
Currently only a few packets are parsed (namely thus I manually assigned a human readable name).  
Since the name is required for generating the according parser all others are currently ignored. 
  
A list of all know packets, their ids, properties and model ids can be found in `resources/pt_packet_schema.json`.  
This dump is from the newest Pro-Tanki client as of 06/05/2023.  
  
# Examples
Currently there are two examples provided.  
The first one is a simple client which can connect to the Pro Tanki server.  
If provided with an login hash it automaticly uses it.  
  
The second example is a proxy which parses & reencodes all send data.  
  
You can run the examples by issueing the following command (assuming `cargo` is installed):
```sh
# Start the proxy server
cargo run --example proxy-server -- -b 127.0.0.1:1234 -t <target address>

# Start the headless client
cargo run --example basic-connection -- -t <target address> -l <login token>
```