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
         var relocate:Function;
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
                     relocate = localProfile._relocateFunction;
                     trace("aaa",localProfile,localProfile._relocateFunction,relocate);
                     relocate(relocations);
                     trace("bbb");
                  }
                  else
                  {
                     MainScene.buyFunction(localProfile.profileId,item.itemId,item.dictId,ItemPlace.SHOP,slot,1,item.cost);
                  }
               }
               else
               {
                  return false;
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

