package survarium.controls
{
   import flash.external.ExternalInterface;
   import scaleform.clik.controls.DragSlot;
   import scaleform.clik.events.DragEvent;
   import survarium.data.ItemData;
   import survarium.interfaces.IInventoryItemRenderer;
   import survarium.interfaces.ILocalPaperDoll;
   import survarium.interfaces.IPaperDoll;
   import survarium.player.LocalPlayerProfile;
   
   public class DragRegion extends DragSlot
   {
      
      protected static var _playerProfile:LocalPlayerProfile;
      
      public function DragRegion()
      {
         super();
      }
      
      public static function set playerProfile(param1:LocalPlayerProfile) : void
      {
         _playerProfile = param1;
      }
      
      override public function handleDropEvent(param1:DragEvent) : Boolean
      {
         var _loc2_:ItemData = param1.dragData as ItemData;
         var _loc3_:ILocalPaperDoll = _playerProfile.profileItems as ILocalPaperDoll;
         var _loc4_:LocalPlayerProfile = _playerProfile;
         var _loc5_:int = _loc3_.getSlot(_loc2_);
         var _loc6_:Array;
         var _loc7_:Function;
         trace("DG::handleDropEvent",_loc5_,_playerProfile);
         if(param1.dragTarget is IInventoryItemRenderer)
         {
            if(_loc2_.descriptor == 100 && _loc2_.category == 25)
            {
               this.tryRelocateCamo(_loc2_);
               return false;
            }
            if(_loc5_ != -1)
            {
               if(!_loc2_.isStack)
               {
                  if(_loc2_.descriptor == 100)
                  {
                     _loc6_ = [{
                        profile:_loc4_.profileId,
                        id:_loc2_.itemId,
                        dict_id:_loc2_.dictId,
                        sourceSlot:100,
                        targetSlot:_loc5_,
                        count:1
                     }];
                     _loc7_ = _loc4_._relocateFunction;
                     trace("aaa",_loc4_,_loc4_._relocateFunction,_loc7_);
                     _loc7_(_loc6_);
                     trace("bbb");
                  }
                  else
                  {
                     MainScene.buyFunction(_loc4_.profileId,_loc2_.itemId,_loc2_.dictId,200,_loc5_,1,_loc2_.cost);
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
      
      private function tryRelocateCamo(param1:ItemData) : void
      {
         var _loc2_:IPaperDoll = _playerProfile.profileItems;
         var _loc3_:ItemData = _loc2_.getSlotAt(2).data as ItemData;
         if(_loc3_ && _loc3_.camoAvailable)
         {
            trace("relocate_upgrade_item_on_drop",_loc3_.itemId,uint(param1.itemId),uint(3));
            ExternalInterface.call("relocate_upgrade_item",_loc3_.itemId,uint(param1.itemId),uint(3));
         }
      }
   }
}

