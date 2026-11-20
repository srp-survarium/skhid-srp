package survarium.controls
{
   import scaleform.clik.controls.TipRenderer;
   
   public class ItemTipRenderer extends TipRenderer
   {
      
      public var itemInfo:ItemInfoRenderer;
      
      public function ItemTipRenderer()
      {
         super();
      }
      
      override protected function invalidateData() : void
      {
         this.itemInfo.data = _data;
         // Drawn as the lobby is, as the inventory's card: its container, the tooltips', is
         // already at MainScene.overlayScale (the lobby's, never below 1).
         this.itemInfo.scaleX = this.itemInfo.scaleY = MainScene.scaleFactor / MainScene.overlayScale;
      }
   }
}

