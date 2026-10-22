package survarium.data
{
   // A move for the exe (`46 00`: relocate_item 0x288D60, apply_profile_items 0x2A5850;
   // lobby_requests.md). The exe reads each field's raw bits whatever the value's type (the count:
   // `movzx edi, word [esp+0x90]` at 0x289234), so a Number, such as an expression `a ? b : c` put
   // in a field, arrives as 0. Every field goes through a typed parameter here
   // (one-pocket-a-type.md, "Revision 28's bug").
   public class Relocation
   {
      public static function move(profile:uint, id:uint, dictId:uint, sourceSlot:uint, targetSlot:uint, count:uint) : Object
      {
         return {
            profile:profile,
            id:id,
            dict_id:dictId,
            sourceSlot:sourceSlot,
            targetSlot:targetSlot,
            count:count
         };
      }
   }
}
