```
Rebase:
  get current_price from price oracle
  load price at last rebase from storage
  for each piggy bank balance:
    balance' = (current_price / last_price) * balance

Unlock
  Initiate unlocking for coins in the piggy bank
  Coins will still be subject to rebases until the unlock period is over

Withdraw
  Withdraw fully unlocked coins once unlock period is over

```

```js
Initialize()
  // Create token factory token and give the contract ownership of it

Rebase()
    current_price = priceOracle()
    last_price = LAST_PRICE.get()
    rebase_multiplier = current_price / last_price
    REBASES.push(rebase_multiplier)

UnlockPiggyBank()
    current_rebase = REBASES.pop()
    
```
