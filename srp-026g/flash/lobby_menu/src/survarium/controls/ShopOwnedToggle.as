package survarium.controls
{
   import flash.events.Event;
   import flash.events.MouseEvent;
   import scaleform.clik.controls.ToolTip;
   import survarium.constants.Colors;
   import survarium.data.LocalizationData;

   // The shop's "not owned only" toggle (shop-owned-mark.md, the owner's of 2026-11-01): the owned
   // mark crossed out, at the right end of the filters' row. On, the shop lists no entry of an item
   // the account owns (InventoryList.hasItem, the mark's own check). Off when the lobby opens; it
   // keeps its state while the lobby is open. Its tooltip is st_srp_shop_not_owned. A press turns
   // it over and says CHANGE.
   public class ShopOwnedToggle extends ToolTip
   {

      public static const SIZE:Number = 28;

      private static const TIP:String = "st_srp_shop_not_owned";

      private var _hidesOwned:Boolean = false;

      public function ShopOwnedToggle()
      {
         super();
         buttonMode = true;
         mouseChildren = false;
         tooltipRendererName = "SrvProfileTipRenderer";
         addEventListener(MouseEvent.CLICK,this.onPress);
         this.paint();
      }

      public function get hidesOwned() : Boolean
      {
         return this._hidesOwned;
      }

      // The text when it shows: the dictionary may come after the toggle is made.
      override protected function showTip(e:Event = null) : void
      {
         tipData = {"name":LocalizationData.getName(TIP)};
         super.showTip(e);
      }

      private function onPress(e:MouseEvent) : void
      {
         this._hidesOwned = !this._hidesOwned;
         this.paint();
         dispatchEvent(new Event(Event.CHANGE));
      }

      // A dark square (it takes the press whole) with the owned mark's check and a stroke across it;
      // on: a green frame and the check and the stroke bright, off: dim.
      private function paint() : void
      {
         var on:Boolean = this._hidesOwned;
         graphics.clear();
         graphics.beginFill(0,on ? 0.75 : 0.45);
         graphics.drawRect(0,0,SIZE,SIZE);
         graphics.endFill();
         graphics.lineStyle(1,on ? Colors.GREEN : Colors.SOFT_GRAY,on ? 1 : 0.6);
         graphics.moveTo(0.5,0.5);
         graphics.lineTo(SIZE - 0.5,0.5);
         graphics.lineTo(SIZE - 0.5,SIZE - 0.5);
         graphics.lineTo(0.5,SIZE - 0.5);
         graphics.lineTo(0.5,0.5);
         graphics.lineStyle(2.5,Colors.GREEN,on ? 1 : 0.45);
         graphics.moveTo(8,14.5);
         graphics.lineTo(12,18.5);
         graphics.lineTo(20,9);
         graphics.lineStyle(2,on ? Colors.RED : Colors.LIGHT_GRAY,on ? 1 : 0.8);
         graphics.moveTo(7,21);
         graphics.lineTo(21,7);
      }
   }
}
