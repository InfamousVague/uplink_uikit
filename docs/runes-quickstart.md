How to create and use runes on our own testnet:
----------------------------------------------------
1 - Install `bitcoind` (pre-compiled) or build from source: https://github.com/bitcoin/bitcoin

2 - Start a node and join the network
```
bitcoind -regtest -txindex -addnode=<node-address>
```
3 - In another terminal, install `ord` (pre-compiled) or build from source: https://github.com/ordinals/ord

4 - Start ord indexer
```
ord --regtest --index-runes server
```
5 - In another terminal, run these commands to create a wallet, produce a receiving address, and mine 101 blocks. (Mined coins need 100 blocks before they can be used. Mining difficulty is disabled, so blocks are mined instantly) This will get you coins to pay for fees in the next steps. Alternatively, you could ask someone to send you some coins.
```
ord --regtest wallet create
ord --regtest wallet receive
bitcoin-cli -regtest generatetoaddress 101 <address>
ord --regtest wallet balance
```

6 - Create the `etch.yaml` and `icon.svg` files in your current directory

`etch.yaml`
```
mode: separate-outputs

etching:
  rune: HOUSE.IS.ON.FIRE
  symbol: '🔥'
  divisibility: 0
  premine: 1000
  supply: 1000000
  terms: 
    amount: 1000
    cap: 999

inscriptions:
  - file: icon.svg
```

`icon.svg`
```
<?xml version="1.0" encoding="UTF-8"?>
<svg viewBox="-1000 -1000 2000 2000" xmlns="http://www.w3.org/2000/svg">
  <style>
    circle {
      fill: black;
      stroke: black;
    }

    @media (prefers-color-scheme: dark) {
      circle {
        fill: white;
        stroke: white;
      }
    }
  </style>
  <circle r="960" fill-opacity="0" stroke-width="80"/>
  <circle r="675" stroke-opacity="0"/>
</svg>
```

7 - Create rune and mine 6 blocks (to prevent front-running, creation is done using commit-reveal, requiring at least 6 blocks before submitting the reveal transaction)
```
ord --regtest wallet batch --fee-rate 1 --batch etch.yaml
bitcoin-cli -regtest generatetoaddress 6 <address>
```

8 - This `etch.yaml` is configured to have a premine of 1000 units, which should be in your wallet. This config also has an open-mint feature (defined under `terms`) where anyone can mint 1000 units to themselves. The config limits this open-mint to happen a max of 999 times. Use the open-mint and mine 1 block:
```
ord --regtest wallet mint --fee-rate 1 --rune <rune> 
bitcoin-cli -regtest generatetoaddress 1 <address>
```

9 - Transfer runes and mine 1 block:
```
ord --regtest wallet send --fee-rate 1 <address> <amount>:<rune>
bitcoin-cli -regtest generatetoaddress 1 <address>
```

10 - View runes in the ord indexer: `localhost/runes`


11 - Some unusual properties of runes:
- You may opt to have the identifier (must be unique) be randomly assigned, or specifically chose it. In this case: `HOUSE.IS.ON.FIRE` The dots are just for readability and don't count as characters in the identifier. so `HOUSEISONFIRE` is the same thing. 
- The length of the identifier must be 13-26 characters long. The minimum length requirement is gradually relaxed over the next 4 years at which point even 1 character identifiers will become available.
- The `symbol` is just 1 character, does not need to be unique, and can be any unicode symbol, including emojis.
