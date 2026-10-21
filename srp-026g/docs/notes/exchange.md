# The exchange window: amounts

The rules the code follows. Code: the exchange window's class,
`flash/lobby_menu/src/survarium/messages/MessageMoneyExchange.as` (`limit`, `affordable`, `onEdit`;
[lobby-source.md](lobby-source.md)).
The owner's rule: 2026-10-20 ("you type 10000 and only have 5500? you get 5500 in the window. Note
if you type 100 gold, it will change to 55 too").

- **Typing more than the Pay balance pays for** gives the most it does, at once, in the field
  typed in; the other field follows. No Max button.
- **Buying gold with 5,500 silver** (rate 100): 10,000 in Pay becomes 5,500; 100 in Receive
  becomes 55.
- **Selling gold:** the same with the gold balance: more gold than the account has becomes all of
  it; more silver in Receive than that gold gives becomes what it gives.
- **Whole gold:** silver is always whole gold. With 5,550 silver the most is 55 gold, 5,500
  silver: anything over 5,500 (5,520 too) becomes 5,500 at once. An amount under the most that is
  not whole gold is evened out when the field is left, as before.
- **The caret** stays at the end of the rewritten number.
- **Not enough silver / gold** (red, Exchange greyed out) is left for the amount the window opens
  with (1 gold) or keeps after a swap, when the balance does not pay for it. A balance of 0 makes
  every typed amount 0.
- **Server:** nothing changes; it still refuses an exchange the account cannot pay for.
