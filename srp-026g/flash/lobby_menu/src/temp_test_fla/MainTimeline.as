package temp_test_fla
{
   import flash.display.MovieClip;
   import survarium.constants.ItemPlace;
   import survarium.data.ItemData;
   
   public dynamic class MainTimeline extends MovieClip
   {
      
      public var tile:InventoryItemHint;
      
      public var itemData:ItemData;
      
      public function MainTimeline()
      {
         super();
         addFrameScript(0,this.frame1);
      }
      
      internal function frame1() : *
      {
         itemData = new ItemData();
         this.itemData.name = "name";
         this.itemData.descriptor = ItemPlace.INVENTORY;
         this.itemData.itemDescription = "itemDescription";
         this.tile.data = this.itemData;
      }
   }
}

