package survarium.player
{
   import flash.display.DisplayObject;
   import flash.display.DisplayObjectContainer;
   import flash.display.Sprite;
   import flash.events.MouseEvent;
   import flash.geom.Rectangle;
   import flash.text.TextField;

   // A slot a skill closes shows the skills' red lock in its middle (the lock of a skill another
   // excludes, without the circle around it: the symbol's first child), with a tip over the slot:
   // the second weapon slot Deep Pockets or Supply Pocket closes, and the pocket a kit gave back
   // for it (PerkSlotMark). A right click on it calls its owner (switching the perk's slot), when
   // given.
   public class SlotLock
   {

      private var _lock:Sprite;

      private var _mark:DisplayObject;

      private var _tip:Sprite;

      private var _area:ReasonsTip;

      private var _pressed:Function;

      public function SlotLock()
      {
         super();
      }

      // Over `slot` in `doll`, its tip `text`; `pressed` called on a click (null: none).
      public function show(doll:DisplayObjectContainer, slot:DisplayObject, text:String, pressed:Function) : void
      {
         if(this._lock == null)
         {
            this.make(doll);
         }
         this._pressed = pressed;
         var bounds:Rectangle = slot.getBounds(doll);
         this._tip.x = bounds.x;
         this._tip.y = bounds.y;
         this._area.graphics.clear();
         this._area.graphics.beginFill(0,0.01);
         this._area.graphics.drawRect(0,0,bounds.width,bounds.height);
         this._area.graphics.endFill();
         doll.addChild(this._tip);
         this._area.validateNow();
         this._area.tooltipRendererName = "SrvProfileTipRendererStyled";
         this._area.tipData = {"name":text};
         this._area.buttonMode = pressed != null;
         doll.addChild(this._lock);
         var own:Rectangle = this._mark.getBounds(this._lock);
         // Clear of the slot's label (the owner, 2026-11-23: the lock over "Weapon"): in the
         // middle of the room above it, else beside it.
         var label:Rectangle = labelOf(slot,doll);
         var x:Number = bounds.x + (bounds.width - own.width) / 2;
         var y:Number = bounds.y + (bounds.height - own.height) / 2;
         if(label != null && label.intersects(new Rectangle(x,y,own.width,own.height)))
         {
            if(label.y - bounds.y >= own.height + 4)
            {
               y = bounds.y + (label.y - bounds.y - own.height) / 2;
            }
            else
            {
               x = label.x - own.width - 4;
               y = label.y + (label.height - own.height) / 2;
            }
         }
         this._lock.x = Math.round(x - own.x);
         this._lock.y = Math.round(y - own.y);
      }

      // The bounds of the slot's shown text (its label), in `doll`'s units; null: none.
      private static function labelOf(slot:DisplayObject, doll:DisplayObject) : Rectangle
      {
         var found:Rectangle = null;
         if(slot is TextField && slot.visible && TextField(slot).text.length > 0)
         {
            // The text itself, not its field (centred in it across, from its top down).
            var field:TextField = TextField(slot);
            var own:Rectangle = field.getBounds(doll);
            var kx:Number = field.width > 0 ? own.width / field.width : 1;
            var ky:Number = field.height > 0 ? own.height / field.height : 1;
            var w:Number = Math.min(own.width,(field.textWidth + 4) * kx);
            found = new Rectangle(own.x + (own.width - w) / 2,own.y,w,Math.min(own.height,(field.textHeight + 4) * ky));
         }
         var box:DisplayObjectContainer = slot as DisplayObjectContainer;
         if(box != null && slot.visible)
         {
            for(var i:int = 0; i < box.numChildren; i++)
            {
               var inner:Rectangle = labelOf(box.getChildAt(i),doll);
               if(inner != null)
               {
                  found = found == null ? inner : found.union(inner);
               }
            }
         }
         return found;
      }

      public function hide() : void
      {
         if(this._lock != null && this._lock.parent != null)
         {
            this._area.hideNow();
            this._lock.parent.removeChild(this._lock);
            this._tip.parent.removeChild(this._tip);
         }
      }

      private function make(doll:DisplayObjectContainer) : void
      {
         this._lock = new SkillLockSpriteRed();
         if(this._lock.numChildren == 2)
         {
            this._lock.getChildAt(0).visible = false;
            this._mark = this._lock.getChildAt(1);
         }
         else
         {
            this._mark = this._lock;
         }
         this._lock.mouseEnabled = false;
         this._lock.mouseChildren = false;
         this._tip = new Sprite();
         this._area = new ReasonsTip();
         this._area.focusable = false;
         this._tip.addChild(this._area);
         this._area.addEventListener(MouseEvent.CLICK,this.onClick);
         // The tip's area is over the slot and is no drag slot: while a button is down it takes
         // no mouse, so a drag released there still lands on the slot under it (which refuses
         // it), not on nothing (which sends the item to the inventory). Back after the drag
         // manager has read where the drag ended (its listener is of priority 0). A click is the
         // press and the release on the area: Flash sends it after the release, the area back.
         if(doll.stage != null)
         {
            doll.stage.addEventListener(MouseEvent.MOUSE_DOWN,this.onPress,true,0,true);
            doll.stage.addEventListener(MouseEvent.MOUSE_UP,this.onRelease,false,-1,true);
         }
      }

      private function onPress(event:MouseEvent) : void
      {
         // A press on the area itself stays its own, so its click comes.
         if(event.target != this._area)
         {
            this._tip.mouseEnabled = this._tip.mouseChildren = false;
         }
      }

      private function onRelease(event:MouseEvent) : void
      {
         this._tip.mouseEnabled = this._tip.mouseChildren = true;
      }

      private function onClick(event:MouseEvent) : void
      {
         if(this._pressed != null && PerkSlotMark.rightClick(event))
         {
            this._pressed();
         }
      }
   }
}
