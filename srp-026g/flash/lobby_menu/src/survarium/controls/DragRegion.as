package survarium.controls
{
   import flash.external.ExternalInterface;
   import scaleform.clik.controls.DragSlot;
   import scaleform.clik.events.DragEvent;
   import survarium.data.Relocation;
   import survarium.data.ItemData;
   import survarium.interfaces.IInventoryItemRenderer;
   import survarium.interfaces.ILocalPaperDoll;
   import survarium.interfaces.IPaperDoll;
   import survarium.player.LocalPaperDoll;
   import survarium.player.LocalPlayerProfile;
   import survarium.constants.CosmeticSlot;
   import survarium.constants.ItemCategory;
   import survarium.constants.ItemPlace;
   import survarium.constants.ProfileSlot;
   
   public class DragRegion extends DragSlot
   {
      
      protected static var _playerProfile:LocalPlayerProfile;
      
      public function DragRegion()
      {
         super();
      }
      
      public static function set playerProfile(value:LocalPlayerProfile) : void
      {
         _playerProfile = value;
      }
      
      override public function handleDropEvent(event:DragEvent) : Boolean
      {
         var item:ItemData = event.dragData as ItemData;
         var paperDoll:ILocalPaperDoll = _playerProfile.profileItems as ILocalPaperDoll;
         var localProfile:LocalPlayerProfile = _playerProfile;
         var slot:int = paperDoll.getSlot(item);
         var relocations:Array;
         trace("DG::handleDropEvent",slot,_playerProfile);
         if(event.dragTarget is IInventoryItemRenderer)
         {
            if(item.descriptor == ItemPlace.INVENTORY && item.category == ItemCategory.CAMOUFLAGES)
            {
               this.tryRelocateCamo(item);
               return false;
            }
            if(slot != -1)
            {
               if(!item.isStack)
               {
                  if(item.descriptor == ItemPlace.INVENTORY)
                  {
                     relocations = [Relocation.move(localProfile.profileId,item.itemId,item.dictId,ItemPlace.INVENTORY,slot,1)];
                     localProfile.relocate(relocations);
                  }
                  // A shop item is bought only by Buy, a double click, or a
                  // drop on the slot it goes in (ItemPaperDollSlot), never
                  // into a slot picked for it here.
               }
            }
            // a pocket item: into its own pocket, else the next free one, filled, as a double
            // click (docs/notes/one-pocket-a-type.md); with no pocket free nothing is replaced
            if(item.isStack && item.descriptor == ItemPlace.INVENTORY && (item.category == ItemCategory.MEDKITS || item.category == ItemCategory.BOOBY_TRAPS || item.category == ItemCategory.GRENADES))
            {
               slot = paperDoll.getSlot(item,true);
               if(slot >= ProfileSlot.QUICK_1 && slot <= ProfileSlot.QUICK_6 && (paperDoll.pocketOf(item.dictId) == slot || !paperDoll.getSlotAt(slot).data))
               {
                  localProfile.relocate([Relocation.move(localProfile.profileId,item.itemId,item.dictId,ItemPlace.INVENTORY,slot,LocalPaperDoll.pocketUnits(item))]);
               }
            }
         }
         return false;
      }
      
      private function tryRelocateCamo(camo:ItemData) : void
      {
         var paperDoll:IPaperDoll = _playerProfile.profileItems;
         var weapon:ItemData = paperDoll.getSlotAt(ProfileSlot.BODY).data as ItemData;
         if(weapon && weapon.camoAvailable)
         {
            trace("relocate_upgrade_item_on_drop",weapon.itemId,uint(camo.itemId),uint(CosmeticSlot.CAMOUFLAGE));
            ExternalInterface.call("relocate_upgrade_item",weapon.itemId,uint(camo.itemId),uint(CosmeticSlot.CAMOUFLAGE));
         }
      }
   }
}

