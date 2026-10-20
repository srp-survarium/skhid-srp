package survarium.paperdoll
{
   import flash.display.Sprite;
   import survarium.interfaces.IPaperDoll;
   
   public class PaperDollHUD extends PaperDollBase implements IPaperDoll
   {
      
      public var itemsBack:Sprite;
      
      public function PaperDollHUD()
      {
         super();
      }
      
      override protected function handleQuickSlots() : void
      {
         super.handleQuickSlots();
         var slots:int = _slotsN + _slotsA;
         this.itemsBack.height = 10 + 27 * slots + 9;
         this.itemsBack.visible = slots > 0;
      }
   }
}

