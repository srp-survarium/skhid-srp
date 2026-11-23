package survarium.constants
{
   // The Controls' five groups of actions, each with its colour (the owner's key board, 2026-11-23:
   // Movement, Fight, Items, Talk, Interface), in the order they are listed. `of` puts each action of
   // the options' controls table (hide_ui_026g.asm's `controls`: the action ids) in its group, as
   // the table's own blocks (#2590): movement; the weapon and grenade, melee, use, drop, hold
   // artefact (fight); the quick slots and the item keys (items); communication (talk); the
   // interface, the screenshots and the lobby's windows (interface).
   public class ActionGroup
   {

      public static const MOVEMENT:ActionGroup = new ActionGroup(0,0x4FB8AD);

      public static const FIGHT:ActionGroup = new ActionGroup(1,0xE3953C);

      public static const ITEMS:ActionGroup = new ActionGroup(2,0x94C451);

      public static const TALK:ActionGroup = new ActionGroup(3,0x5EA3EA);

      public static const INTERFACE:ActionGroup = new ActionGroup(4,0xA397C4);

      public static const ALL:Array = [MOVEMENT,FIGHT,ITEMS,TALK,INTERFACE];

      // jump, crouch, crouch toggle, sprint, sprint toggle, forward, backward, strafe left and right
      private static const MOVEMENT_IDS:Array = [0x04,0x05,0x06,0x07,0x08,0x0B,0x0C,0x0D,0x0E];

      // the quick slots 1-6, patch 149's sixteen item keys
      private static const ITEMS_IDS:Array = [0x39,0x3A,0x3B,0x3C,0x3D,0x3E];

      private static const FIRST_ITEM_KEY:uint = 0x4A;

      private static const LAST_ITEM_KEY:uint = 0x59;

      // spot, chat, send message, send to, radio, patch 66's talk nearby
      private static const TALK_IDS:Array = [0x22,0x3F,0x45,0x46,0x47,0x37];

      // the lobby's five windows, patch 152's screenshots and the hide interface key
      private static const INTERFACE_IDS:Array = [0x40,0x41,0x42,0x43,0x44,0x19,0x1A,0x1B];

      // Its place in ALL: the index of its name in root.srp_controls_texts' groups.
      public var index:int;

      public var colour:uint;

      public function ActionGroup(index:int, colour:uint)
      {
         super();
         this.index = index;
         this.colour = colour;
      }

      // The rest, the weapon's and the hands' keys (fire, aim, breath, reload, weapons, grenade,
      // melee, use, drop, patch 78's hold artefact), fight.
      public static function of(actionId:uint) : ActionGroup
      {
         if(MOVEMENT_IDS.indexOf(actionId) != -1)
         {
            return MOVEMENT;
         }
         if(ITEMS_IDS.indexOf(actionId) != -1 || actionId >= FIRST_ITEM_KEY && actionId <= LAST_ITEM_KEY)
         {
            return ITEMS;
         }
         if(TALK_IDS.indexOf(actionId) != -1)
         {
            return TALK;
         }
         if(INTERFACE_IDS.indexOf(actionId) != -1)
         {
            return INTERFACE;
         }
         return FIGHT;
      }
   }
}
