# Quests of a complete faction pay experience

The owner, 2026-11-08: "once you maxed out experience for a faction, that experience should go into
level", "that should be telegraphed with missions", "after you complete the faction, the quests
that are rolled give only experience and not faction experience", "make sure user UI doesn't show +0".

- A faction is complete when the account's reputation with it reaches its top level's threshold
  (`factions_dict`, its last level: Scavengers level 5 at 3000).
- A complete faction's quests pay their reputation as experience: the same number, 1:1, with the
  same scaling (the daily doubling) as the reputation would have had. Their money is unchanged.
- The experience is paid with the match's, so the levels it reaches pay their silver as any level does.
- Decided when the quest is paid: a quest finished in the match that completes the faction is
  paid after the match's own reputation, so it already pays experience.
- The match's own reputation (its rows, premium, daily, items) is unchanged: it still goes to the
  faction, past the top (to the wire's 65535) as before.
- Missions UI (the lobby movie): a complete faction's quest reward shows the experience icon in
  place of the reputation icon, with the quest's number. Nothing shows a "+0" reputation; there is
  one entry, not two. The same in the quest-completed notifications and the quest info message.
- The results screen has no quest-experience line: the experience is not on it (its quest
  reputation line is 0, as for any match without a quest).
