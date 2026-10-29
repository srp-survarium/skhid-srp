package survarium.player
{
   import flash.display.MovieClip;
   import flash.events.Event;
   import scaleform.clik.controls.TipRenderer;
   import survarium.controls.ItemInfoRenderer;
   import survarium.buttons.ModificatorListRenderer;
   import survarium.data.IconContainer;

   // Reuse the weapon card's actual frame, critical-damage divider and rows.
   // No duplicate drawing, replacement icons or separate font definitions.
   public class WeightTipRenderer extends TipRenderer
   {
      private var _background:MovieClip;
      private var _body:MovieClip;
      private var _heading:MovieClip;
      private var _rows:Array = [];
      private var _skin:ItemInfoRenderer;

      public function WeightTipRenderer()
      {
         super();
         _skin = ItemInfoRenderer.warningSkin();
         addChild(_skin);
         alpha = 0;
         addEventListener(Event.ADDED_TO_STAGE,onAdded,false,0,true);
         addEventListener(Event.REMOVED_FROM_STAGE,onRemoved,false,0,true);
      }

      private function onAdded(e:Event) : void
      {
         addEventListener(Event.ENTER_FRAME,prepare,false,0,true);
      }

      private function onRemoved(e:Event) : void { removeEventListener(Event.ENTER_FRAME,prepare); }

      override protected function invalidateData() : void
      {
         if(stage != null) addEventListener(Event.ENTER_FRAME,prepare,false,0,true);
      }

      private function prepare(e:Event) : void
      {
         if(_data == null) return;
         var effects:IEffectRows = _data.effects as IEffectRows;
         if(effects == null) throw new Error("WeightTipRenderer: missing effects");
         // New native timeline instances populate nested fields on their first
         // frame, not during construction or ADDED_TO_STAGE (unlike Ruffle).
         if(_skin != null)
         {
            if(_skin.criticalSeparator == null || _skin.criticalSeparator.textField == null) return;
            _background = _skin.background;
            _body = _skin.topBlackContainer;
            _heading = _skin.criticalSeparator;
            addChild(_background);
            addChild(_body);
            addChild(_heading);
            removeChild(_skin);
            _skin = null;
            for each(var item:Object in effects.rows())
            {
               var nativeRow:ModificatorListRenderer = new SrvModifierLabeledRenderer();
               addChild(nativeRow);
               _rows.push(nativeRow);
            }
            return; // Let the row timelines initialize too.
         }
         for each(var pending:ModificatorListRenderer in _rows)
            if(pending.textFieldLabel == null || pending.iconContainer == null) return;
         _heading.textField.text = effects.title;
         var icons:IconContainer = IconContainer.getInstance("modifiers");
         if(icons == null) throw new Error("WeightTipRenderer: missing modifier atlas");
         var w:Number = 250;
         _heading.y = 8 - _heading.textField.y;
         var y:Number = 12 + _heading.textField.height;
         var entries:Array = effects.rows();
         for(var i:int = 0; i < _rows.length; i++)
         {
            var row:ModificatorListRenderer = _rows[i];
            var entry:Object = entries[i];
            row.validateNow();
            row.data = {icon:icons.getIcon(entry.icon),label:entry.label};
            row.validateNow();
            row.x = 10;
            row.y = y;
            // These are the native row's icon scale, font and text margins.
            w = Math.max(w,row.x + row.textFieldLabel.x + row.textFieldLabel.textWidth + 14);
            y += Math.max(32,row.textFieldLabel.textHeight + 8);
         }
         _background.x = _background.y = 0;
         _background.width = Math.ceil(w);
         _background.height = Math.ceil(y + 10);
         _body.x = 2;
         _body.y = 2;
         _body.width = _background.width - 4;
         _body.height = _background.height - 4;
         _heading.x = (_background.width - _heading.width) / 2;
         alpha = 1;
         removeEventListener(Event.ENTER_FRAME,prepare);
         dispatchEvent(new Event(Event.RESIZE));
      }
   }
}
