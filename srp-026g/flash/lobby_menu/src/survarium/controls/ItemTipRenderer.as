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
         this.itemInfo.scaleX = this.itemInfo.scaleY = MainScene.scaleFactor;
      }
   }
}

