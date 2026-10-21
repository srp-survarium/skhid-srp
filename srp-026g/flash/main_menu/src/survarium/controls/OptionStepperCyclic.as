package survarium.controls
{
   import flash.display.MovieClip;
   import flash.text.TextField;
   import scaleform.clik.constants.InvalidationType;
   import scaleform.clik.controls.OptionStepper;
   import scaleform.clik.events.ComponentEvent;
   import scaleform.clik.utils.Constraints;
   
   public class OptionStepperCyclic extends OptionStepper
   {
      
      public var glow:MovieClip;
      
      protected var _cyclic:Boolean = false;
      
      protected var _label:String = "defLabel";
      
      public var textField2:TextField;
      
      public function OptionStepperCyclic()
      {
         super();
         _selectedIndex = -1;
      }
      
      public function blink() : void
      {
         if(this.glow)
         {
            this.glow.gotoAndPlay(2);
         }
      }
      
      [Inspectable(defaultValue="false")]
      public function get cyclic() : Boolean
      {
         return this._cyclic;
      }
      
      public function set cyclic(value:Boolean) : void
      {
         _cyclic = false;
      }
      
      override protected function configUI() : void
      {
         super.configUI();
         if(!_constraintsDisabled)
         {
            constraints.addElement("textField2",this.textField2,Constraints.RIGHT);
         }
         nextBtn.enabled = _newSelectedIndex + 1 < dataProvider.length;
         prevBtn.enabled = _newSelectedIndex > 0;
      }
      
      override public function set selectedIndex(value:int) : void
      {
         if(_selectedIndex != -1 && _selectedIndex != value)
         {
            this.blink();
         }
         super.selectedIndex = value;
         nextBtn.enabled = value + 1 < dataProvider.length;
         prevBtn.enabled = value > 0;
         this.updateLabel();
      }
      
      public function set label(value:String) : void
      {
         _label = value;
         this.updateLabel();
      }
      
      public function get label() : String
      {
         return this._label;
      }
      
      override protected function draw() : void
      {
         if(isInvalid(InvalidationType.SELECTED_INDEX))
         {
            updateSelectedIndex();
         }
         if(isInvalid(InvalidationType.STATE))
         {
            if(_newFrame)
            {
               gotoAndPlay(_newFrame);
               _newFrame = null;
            }
            updateAfterStateChange();
            dispatchEvent(new ComponentEvent(ComponentEvent.STATE_CHANGE));
            invalidate(InvalidationType.DATA);
         }
         if(isInvalid(InvalidationType.DATA))
         {
            refreshData();
         }
      }
      
      override protected function updateLabel() : void
      {
         if(textField != null)
         {
            textField.text = this._label;
         }
         if(selectedItem == null)
         {
            return;
         }
         if(this.textField2 != null)
         {
            this.textField2.text = itemToLabel(selectedItem);
         }
      }
   }
}

