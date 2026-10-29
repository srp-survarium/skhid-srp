package survarium.player
{
   import flash.events.Event;
   import flash.geom.Point;
   import scaleform.clik.controls.ToolTip;

   // Native item-card critical-damage presentation, with normal hover timing.
   public class WeightTip extends ToolTip
   {
      public function WeightTip()
      {
         super();
         _tooltipRenderer = WeightTipRenderer;
      }
      public function updateEffects(effects:WeightEffects) : void
      {
         var shown:Boolean = _tip_content != null && _tip_content.parent != null;
         if(shown || effects == null) handleDeleted(null);
         tipEnabled = effects != null;
         tipData = effects == null ? null : {effects:effects};
         if(shown && tipEnabled) showTip();
      }
      override protected function showTip(e:Event = null) : void
      {
         super.showTip(e);
         if(_tip_content == null || _tip_content.parent == null) return;
         _tip_content.addEventListener(Event.RESIZE,placeTip,false,0,true);
         placeTip();
      }
      private function placeTip(e:Event = null) : void
      {
         if(_tip_content == null || _tip_content.parent == null || stage == null) return;
         var corner:Point = tipContainer.globalToLocal(new Point(stage.stageWidth,stage.stageHeight));
         var mouse:Point = tipContainer.globalToLocal(new Point(stage.mouseX,stage.mouseY));
         _tip_content.x = mouse.x + 20;
         _tip_content.y = mouse.y + 20;
         if(_tip_content.x + _tip_content.width > corner.x)
            _tip_content.x = Math.max(4,mouse.x - _tip_content.width + 4);
         if(_tip_content.y + _tip_content.height > corner.y)
            _tip_content.y = Math.max(4,mouse.y - _tip_content.height - 4);
      }
   }
}
