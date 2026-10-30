package survarium.player
{
   import flash.display.MovieClip;
   import flash.events.Event;
   import flash.text.StyleSheet;
   import flash.text.TextFormat;
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
      // One a row entry; null where the entry is a section's heading ({heading}).
      private var _rows:Array = [];
      private var _skin:ItemInfoRenderer;
      // A section's heading: the same divider as the title, from another copy of the
      // card's skin (waiting for its timeline, as the first), one a {heading} entry.
      private var _sectionSkins:Array = [];
      private var _sections:Array = [];

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

      // The heading's value in its style's colour, by TextFormat over its characters:
      // the rows' own style sheet (ModificatorListRenderer's), so heading and rows agree.
      private function colourTitle(effects:IEffectRows) : void
      {
         var part:String = effects.titleValue;
         if(part == null || _heading.textField.styleSheet != null || _rows.length == 0) return;
         var rows:StyleSheet = (_rows[0] as ModificatorListRenderer).textFieldLabel.styleSheet;
         if(rows == null) return;
         var at:int = _heading.textField.text.lastIndexOf(part);
         var style:Object = rows.getStyle(effects.titleStyle);
         if(at < 0 || style == null || style.color == null) return;
         var format:TextFormat = new TextFormat();
         format.color = uint("0x" + String(style.color).replace("#",""));
         _heading.textField.setTextFormat(format,at,at + part.length);
      }

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
               if(item.heading != null)
               {
                  var section:ItemInfoRenderer = ItemInfoRenderer.warningSkin();
                  addChild(section);
                  _sectionSkins.push(section);
                  _rows.push(null);
                  continue;
               }
               var nativeRow:ModificatorListRenderer = new SrvModifierLabeledRenderer();
               addChild(nativeRow);
               _rows.push(nativeRow);
            }
            return; // Let the row timelines initialize too.
         }
         for each(var pending:ModificatorListRenderer in _rows)
            if(pending != null && (pending.textFieldLabel == null || pending.iconContainer == null)) return;
         for each(var skin:ItemInfoRenderer in _sectionSkins)
            if(skin.criticalSeparator == null || skin.criticalSeparator.textField == null) return;
         for each(skin in _sectionSkins)
         {
            var divider:MovieClip = skin.criticalSeparator;
            addChild(divider);
            removeChild(skin);
            _sections.push(divider);
         }
         _sectionSkins = [];
         _heading.textField.text = effects.title;
         colourTitle(effects);
         var icons:IconContainer = IconContainer.getInstance("modifiers");
         if(icons == null) throw new Error("WeightTipRenderer: missing modifier atlas");
         var w:Number = 250;
         _heading.y = 8 - _heading.textField.y;
         var y:Number = 12 + _heading.textField.height;
         var entries:Array = effects.rows();
         var nextSection:int = 0;
         for(var i:int = 0; i < _rows.length; i++)
         {
            var entry:Object = entries[i];
            if(entry.heading != null)
            {
               var heading:MovieClip = _sections[nextSection++];
               heading.textField.text = entry.heading;
               heading.y = y + 6 - heading.textField.y;
               y += heading.textField.height + 10;
               continue;
            }
            var row:ModificatorListRenderer = _rows[i];
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
         for each(var sectionHeading:MovieClip in _sections) sectionHeading.x = _heading.x;
         alpha = 1;
         removeEventListener(Event.ENTER_FRAME,prepare);
         dispatchEvent(new Event(Event.RESIZE));
      }
   }
}
