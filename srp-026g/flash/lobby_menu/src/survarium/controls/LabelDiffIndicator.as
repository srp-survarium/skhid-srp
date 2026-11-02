package survarium.controls
{
   import flash.text.TextField;
   import flash.text.TextFieldAutoSize;
   import flash.text.TextFormat;
   import flash.text.TextFormatAlign;
   import scaleform.clik.constants.ConstrainMode;
   import scaleform.clik.constants.InvalidationType;
   import scaleform.clik.utils.Constraints;
   import survarium.Util;
   import survarium.constants.Colors;
   import survarium.constants.PropertyDirection;
   import survarium.player.MatchEffects;
   
   public class LabelDiffIndicator extends DiffIndicator
   {
      
      public var textFieldValue:TextField;
      
      public var textFieldLabel:TextField;
      
      public var textFieldDiff:TextField;
      
      protected var _prefix:String = "";
      
      protected var _postfix:String = "";
      
      protected var _label:String = "";

      // Text-only weapon details still use the native row's embedded fonts.
      // Null keeps the ordinary numeric comparison behaviour.
      private var _detailValue:String = null;

      // A weapon card's row while Alt is held, its value changed by its modifications, the kit or
      // the skills: {text: "base -> in a match" with its unit, in place of the value (the bar
      // keeps the value), verdict: MatchEffects.BETTER or WORSE}; null shows the value.
      private var _change:Object = null;

      // A detail row's value's verdict, where it is such a change (MatchEffects.BETTER or WORSE);
      // null otherwise.
      private var _detailVerdict:String = null;

      // The value field's own width (unscaled) and line, which a wider detail value grows from;
      // 0 until the first layout.
      private var _valueWidth:Number = 0;

      private var _valueY:Number = 0;

      // A detail row's value: on the label's line (0), or on the line under it (1) where it leaves
      // the label too little room for one line (fitWideValue).
      private var _valueLine:int = 0;

      // The lines a detail row takes (WeaponDetailsView): its label's, and the value's own line
      // under it.
      public function get detailLines() : int
      {
         return textFieldLabel.numLines + _valueLine;
      }

      // `text` null: the row's own value.
      public function setChange(text:String, verdict:String) : void
      {
         _change = text == null ? null : {text:text,verdict:verdict};
         invalidateData();
      }

      // A text field's width in its parent's units: a scaled field's width setter may take its
      // own units (Ruffle does), so the setter is corrected by what the getter reads.
      private static function resize(field:TextField, width:Number) : void
      {
         field.width = width;
         if(field.width > 0 && Math.abs(field.width - width) > 0.01) field.width = width * width / field.width;
      }

      // A detail value wider than its field (a changed range held Alt, "1.8…2 -> 1.73…1.92")
      // takes the room to its left, its right edge kept; where the label has too little room
      // for one line beside it ("base -> in a match" held Alt), the value takes the line under it.
      // The constraints counter-scale the field (scaleX) after its first layout, and x is the
      // native column's again at each layout.
      private function fitWideValue() : void
      {
         var scale:Number = textFieldValue.scaleX;
         if(_valueWidth == 0)
         {
            _valueWidth = textFieldValue.width / scale;
            _valueY = textFieldValue.y;
         }
         var natural:Number = _valueWidth * scale;
         var wanted:Number = Math.max(natural,textFieldValue.textWidth * Math.max(1,scale) + 5);
         resize(textFieldValue,wanted);
         textFieldValue.x += natural - wanted;
         textFieldLabel.autoSize = TextFieldAutoSize.NONE;
         textFieldLabel.wordWrap = false;
         var labelWidth:Number = textFieldLabel.textWidth * Math.max(1,textFieldLabel.scaleX);
         var valueStart:Number = textFieldValue.x + wanted - textFieldValue.textWidth * Math.max(1,scale) - 4;
         // With a margin: Scaleform lays the embedded Calibri out close to, not exactly as, this.
         _valueLine = textFieldLabel.x + labelWidth * 1.05 + 6 > valueStart ? 1 : 0;
         textFieldValue.y = _valueY + _valueLine * textFieldLabel.getLineMetrics(0).height * textFieldLabel.scaleY;
      }

      public function setDetailText(label:String, value:String, verdict:String = null) : void
      {
         _detailVerdict = verdict;
         _label = label;
         _detailValue = value;
         invalidateData();
      }
      
      public function LabelDiffIndicator()
      {
         super();
      }
      
      public function get label() : String
      {
         return this._label;
      }
      
      public function set label(value:String) : void
      {
         _label = value;
         invalidateData();
      }
      
      public function get prefix() : String
      {
         return this._prefix;
      }
      
      public function set prefix(value:String) : void
      {
         _prefix = value;
         invalidateData();
      }
      
      public function get postfix() : String
      {
         return this._postfix;
      }
      
      public function set postfix(value:String) : void
      {
         _postfix = value;
         invalidateData();
      }
      
      override protected function configUI() : void
      {
         super.configUI();
         if(!constraints)
         {
            constraints = new Constraints(this,ConstrainMode.COUNTER_SCALE);
         }
         this.textFieldLabel.x = -2;
         if(constraints)
         {
            constraints.addElement("textFieldValue",this.textFieldValue,Constraints.RIGHT);
            constraints.addElement("textFieldLabel",this.textFieldLabel,Constraints.LEFT);
         }
         this.textFieldLabel.defaultTextFormat = this.textFieldLabel.getTextFormat();
         this.textFieldValue.defaultTextFormat = this.textFieldValue.getTextFormat();
      }
      
      override protected function draw() : void
      {
         super.draw();
         if(isInvalid(InvalidationType.SIZE))
         {
            if(constraints)
            {
               constraints.update(_width,_height);
            }
         }
         if(_detailValue != null) updatePosition();
      }
      
      override protected function updatePosition() : void
      {
         var diffText:String;
         var betterColor:uint;
         var worseColor:uint;
         super.updatePosition();
         if(this._label)
         {
            this.textFieldLabel.text = this._label;
         }
         if(_realDiff == 0)
         {
            diffText = "";
         }
         else
         {
            diffText = (_realDiff > 0 ? "+" : "") + Util.getNumberString(_realDiff).toString();
         }
         this.textFieldValue.defaultTextFormat = this.textFieldValue.getTextFormat();
         this.textFieldDiff.defaultTextFormat = this.textFieldDiff.getTextFormat();
         this.textFieldValue.textColor = 15911311;
         this.textFieldDiff.text = "";
         if(_realDiff == 0)
         {
            this.textFieldDiff.textColor = 13356477;
            if(this._prefix == "-" && value != 0)
            {
               _prefix = "";
            }
            this.textFieldValue.text = this._prefix + Util.getNumberString(value) + this._postfix;
         }
         else
         {
            betterColor = Colors.GREEN_DIFF;
            worseColor = Colors.RED_DIFF;
            if(_direction == PropertyDirection.LOWER_IS_BETTER)
            {
               this.textFieldValue.textColor = _realDiff < 0 ? worseColor : betterColor;
            }
            else
            {
               this.textFieldValue.textColor = _realDiff < 0 ? betterColor : worseColor;
            }
            if(this._prefix == "-" && _realOther != 0)
            {
               _prefix = "";
            }
            this.textFieldValue.text = this._prefix + Util.getNumberString(_realValue) + this._postfix;
         }
         if(_change != null) this.textFieldValue.text = _change.text;
         if(this.textFieldDiff.text.length)
         {
            this.textFieldValue.x = this.textFieldDiff.x + this.textFieldDiff.width - this.textFieldValue.width - this.textFieldDiff.textWidth - 4;
         }
         else
         {
            this.textFieldValue.x = this.textFieldDiff.x + 4;
         }
         // Ctrl keeps the values but removes comparison highlighting.
         var neutralComparison:Boolean = MainScene.isCtrl;
         if(neutralComparison) this.textFieldValue.textColor = 15911311;
         mc_diff_positive.visible = mc_diff_negative.visible = !neutralComparison;
         if(mc_diff_positive_ghost) mc_diff_positive_ghost.visible = !neutralComparison;
         if(mc_diff_negative_ghost) mc_diff_negative_ghost.visible = !neutralComparison;
         if(_detailValue != null)
         {
            // Keep the original row's typography, colors and value alignment.
            // Only replace its numeric string; these base details have no bars.
            textFieldValue.text = _detailValue;
            textFieldDiff.visible = false;
            mc_value.visible = mc_diff_positive.visible = mc_diff_negative.visible = false;
            if(mc_diff_positive_ghost) mc_diff_positive_ghost.visible = false;
            if(mc_diff_negative_ghost) mc_diff_negative_ghost.visible = false;
            if(!_detailValue.length)
               textFieldLabel.width = textFieldValue.x + textFieldValue.width - textFieldLabel.x;
            else
            {
               // A long (translated) label wraps before the value's text
               // instead of running under it; WeaponDetailsView gives the
               // row its lines.
               var align:String = textFieldValue.getTextFormat().align;
               if(align == TextFormatAlign.RIGHT) fitWideValue();
               var valueStart:Number = textFieldValue.x;
               if(_valueLine == 1) valueStart += textFieldValue.width + 6;
               else if(align == TextFormatAlign.RIGHT) valueStart += textFieldValue.width - textFieldValue.textWidth - 4;
               else if(align == TextFormatAlign.CENTER) valueStart += (textFieldValue.width - textFieldValue.textWidth) / 2 - 2;
               textFieldLabel.width = Math.max(1,valueStart - 6 - textFieldLabel.x);
            }
            textFieldLabel.multiline = textFieldLabel.wordWrap = true;
            textFieldLabel.autoSize = TextFieldAutoSize.LEFT;
         }
         var verdict:String = _detailValue != null ? _detailVerdict : _change != null ? _change.verdict : null;
         if(verdict != null) colourChange(verdict,neutralComparison);
      }

      // Held Alt, a changed value "original -> current" (alt-weapon-stats.md): the original and
      // the arrow in the value's neutral colour, the current green when it is better for the
      // player, red when worse, the comparison's own colours; Ctrl leaves it neutral.
      private function colourChange(verdict:String, neutral:Boolean) : void
      {
         textFieldValue.textColor = 15911311;
         var text:String = textFieldValue.text;
         var at:int = text.lastIndexOf(MatchEffects.ARROW);
         if(neutral || at < 0) return;
         var format:TextFormat = new TextFormat();
         format.color = verdict == MatchEffects.BETTER ? Colors.GREEN_DIFF : Colors.RED_DIFF;
         textFieldValue.setTextFormat(format,at + MatchEffects.ARROW.length,text.length);
      }
   }
}

