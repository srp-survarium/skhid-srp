package survarium.upgrade
{
   import com.greensock.TweenLite;
   import com.greensock.easing.Back;
   import flash.display.MovieClip;
   import flash.display.Sprite;
   import flash.events.Event;
   import flash.text.TextField;
   import scaleform.clik.constants.InvalidationType;
   import scaleform.clik.controls.Button;
   import scaleform.clik.controls.DragSlot;
   import scaleform.clik.core.UIComponent;
   import scaleform.clik.data.ListData;
   import scaleform.clik.events.ButtonEvent;
   import scaleform.clik.events.DragEvent;
   import scaleform.clik.interfaces.IListItemRenderer;
   
   public class UpgradeSlot extends DragSlot implements IListItemRenderer
   {
      
      public static const UPDATED:String = "updated";
      
      protected var _index:uint = 0;
      
      protected var _owner:UIComponent;
      
      protected var _selectable:Boolean = true;
      
      protected var _selected:Boolean = false;
      
      protected var _highlighted:Boolean = false;
      
      protected var _label:String = "";
      
      protected var _transitionMap:Object;
      
      public var patternMC:MovieClip;
      
      public var textFieldLabel:TextField;
      
      public var textField:TextField;
      
      public var maskSymbol:Sprite;
      
      public var maskSymbol2:Sprite;
      
      public var border:MovieClip;
      
      public var spriteAnchor2:Sprite;
      
      public var selectedBorder:Sprite;
      
      public var btnChange:Button;
      
      public var btnDrop:Button;
      
      protected var posY:int;
      
      public var startData:Object = null;
      
      protected var _lastData:Object = null;
      
      protected var statesHighlighted:String = "highlighted_";
      
      protected var statesFilledHighlighted:String = "highlighted_filled_";
      
      protected var statesFilled:String = "filled_";
      
      public function UpgradeSlot()
      {
         super();
         _stateMap = {
            up:["up"],
            over:["over"],
            out:["up"],
            down:["up"],
            disabled:["up"]
         };
         _newFrame = "up";
         this.patternMC.alpha = 0.5;
         this.setTransitionSize(45,74);
         var anchor:MovieClip = getChildByName("spriteAnchor") as MovieClip;
         if(anchor && contentCanvas)
         {
            contentCanvas = new Sprite();
            anchor.addChild(contentCanvas);
            if(anchor.x == -30)
            {
               anchor.x -= 9;
               anchor.y -= 7;
               this.patternMC.y = 19;
            }
            else
            {
               this.patternMC.y = 33;
            }
            this.patternMC.x = -17;
            this.patternMC.width = this.patternMC.height = 140;
            this.patternMC.mask = this.maskSymbol2;
            anchor.mask = this.maskSymbol;
            setChildIndex(this.textField,0);
         }
         this.textField.mouseEnabled = false;
         this.textFieldLabel.mouseEnabled = false;
         anchor.y += 13;
      }
      
      public function setTransitionSize(emptySize:uint, filledSize:uint) : void
      {
         var emptySide:int = (emptySize);
         var filledSide:int = (filledSize);
         var hoverGrow:Number = 10;
         var emptyY:Number = filledSide - emptySide / 2;
         var filledY:Number = emptyY;
         posY = emptyY;
         var centerX:Number = 37;
         var borderPad:Number = 2;
         _transitionMap = {
            up:[{
               target:this.maskSymbol,
               options:{
                  x:centerX,
                  y:emptyY,
                  width:emptySide,
                  height:emptySide
               }
            },{
               target:this.border,
               options:{
                  x:centerX,
                  y:emptyY,
                  width:emptySide + borderPad,
                  height:emptySide + borderPad
               }
            }],
            over:[{
               target:this.maskSymbol,
               options:{
                  x:centerX,
                  y:emptyY,
                  width:emptySide + hoverGrow,
                  height:emptySide + hoverGrow
               }
            },{
               target:this.border,
               options:{
                  x:centerX,
                  y:emptyY,
                  width:emptySide + hoverGrow + borderPad,
                  height:emptySide + hoverGrow + borderPad
               }
            }],
            highlighted_up:[{
               target:this.maskSymbol,
               options:{
                  x:centerX,
                  y:emptyY,
                  width:emptySide + hoverGrow,
                  height:emptySide + hoverGrow
               }
            },{
               target:this.border,
               options:{
                  x:centerX,
                  y:emptyY,
                  width:emptySide + hoverGrow + borderPad,
                  height:emptySide + hoverGrow + borderPad
               }
            }],
            highlighted_over:[{
               target:this.maskSymbol,
               options:{
                  x:centerX,
                  y:filledY,
                  width:filledSide + hoverGrow,
                  height:filledSide + hoverGrow
               }
            },{
               target:this.border,
               options:{
                  x:centerX,
                  y:filledY,
                  width:filledSide + hoverGrow + borderPad,
                  height:filledSide + hoverGrow + borderPad
               }
            }],
            filled_up:[{
               target:this.maskSymbol,
               options:{
                  x:centerX,
                  y:filledY,
                  width:filledSide,
                  height:filledSide
               }
            },{
               target:this.border,
               options:{
                  x:centerX,
                  y:filledY,
                  width:filledSide + borderPad,
                  height:filledSide + borderPad
               }
            }],
            filled_over:[{
               target:this.maskSymbol,
               options:{
                  x:centerX,
                  y:filledY,
                  width:filledSide,
                  height:filledSide
               }
            },{
               target:this.border,
               options:{
                  x:centerX,
                  y:filledY,
                  width:filledSide + borderPad,
                  height:filledSide + borderPad
               }
            }],
            highlighted_filled_up:[{
               target:this.maskSymbol,
               options:{
                  x:centerX,
                  y:filledY,
                  width:filledSide + hoverGrow,
                  height:filledSide + hoverGrow
               }
            },{
               target:this.border,
               options:{
                  x:centerX,
                  y:filledY,
                  width:filledSide + hoverGrow + borderPad,
                  height:filledSide + hoverGrow + borderPad
               }
            }],
            highlighted_filled_over:[{
               target:this.maskSymbol,
               options:{
                  x:centerX,
                  y:filledY,
                  width:filledSide + hoverGrow,
                  height:filledSide + hoverGrow
               }
            },{
               target:this.border,
               options:{
                  x:centerX,
                  y:filledY,
                  width:filledSide + hoverGrow + borderPad,
                  height:filledSide + hoverGrow + borderPad
               }
            }]
         };
      }
      
      private function onBorderResize(event:Event) : void
      {
         var padding:uint;
         this.textFieldLabel.y = this.posY + this.maskSymbol.height / 2 + 5;
         this.textField.y = this.textFieldLabel.y;
         this.maskSymbol2.x = this.maskSymbol.x;
         this.maskSymbol2.y = this.maskSymbol.y;
         this.maskSymbol2.width = this.maskSymbol.width;
         this.maskSymbol2.height = this.maskSymbol.height;
         if(this.btnChange)
         {
            this.btnChange.x = 34 + this.maskSymbol.width / 2 + 1;
            this.btnDrop.x = this.btnChange.x + 16;
         }
         if(this.selectedBorder)
         {
            padding = 5;
            this.selectedBorder.width = this.maskSymbol.width + padding * 2;
            this.selectedBorder.height = this.maskSymbol.height + padding * 2;
            this.selectedBorder.x = this.maskSymbol.x - this.selectedBorder.width / 2;
            this.selectedBorder.y = this.maskSymbol.y - this.selectedBorder.height / 2;
         }
      }
      
      public function get owner() : UIComponent
      {
         return this._owner;
      }
      
      public function set owner(value:UIComponent) : void
      {
         _owner = value;
      }
      
      public function set label(value:String) : void
      {
         _label = value;
         invalidateData();
      }
      
      public function get selectable() : Boolean
      {
         return this._selectable;
      }
      
      public function set selectable(value:Boolean) : void
      {
         _selectable = value;
      }
      
      public function get highlighted() : Boolean
      {
         return this._highlighted;
      }
      
      public function set highlighted(value:Boolean) : void
      {
         if(this._highlighted == value)
         {
            return;
         }
         _highlighted = value;
         this.setState("up");
         invalidateState();
         invalidateData();
      }
      
      public function get selected() : Boolean
      {
         return this._selected;
      }
      
      public function set selected(value:Boolean) : void
      {
         _selected = value;
         this.setState("up");
      }
      
      override public function get data() : Object
      {
         return _data;
      }
      
      override public function set data(value:Object) : void
      {
         if(_data != value)
         {
            _data = value;
            stackTrace("US::setdata");
         }
         this.setState("up");
         invalidateData();
      }
      
      public function get index() : uint
      {
         return this._index;
      }
      
      public function set index(value:uint) : void
      {
         _index = value;
         _descriptor = value;
      }
      
      public function setListData(listData:ListData) : void
      {
         index = listData.index;
         selected = listData.selected;
      }
      
      public function setData(newData:Object) : void
      {
         _lastData = this.data;
         this.data = newData;
         dispatchEvent(new Event(UPDATED,true));
         _lastData = null;
      }
      
      public function getLastData() : Object
      {
         return this._lastData;
      }
      
      public function dropSlot() : void
      {
         if(!this.data)
         {
            return;
         }
         this.setData(null);
      }
      
      override public function handleDragStartEvent(event:DragEvent) : void
      {
         this.setState("over");
         content = null;
         this.textField.text = "  ";
      }
      
      override public function handleDragEndEvent(event:DragEvent, wasValidDrop:Boolean) : void
      {
         trace("UT::handleDragEndEvent::",event.dropTarget);
         if(event.dropTarget != this)
         {
            this.dropSlot();
            dispatchEvent(event);
         }
         else
         {
            data = event.dragData;
            if(this.btnChange)
            {
               this.btnChange.enabled = false;
            }
            mouseChildren = false;
         }
         trace("UT::handleDragEndEvent::end");
      }
      
      override public function handleDropEvent(event:DragEvent) : Boolean
      {
         if(enabled == false || this.highlighted == false)
         {
            return false;
         }
         dispatchEvent(event);
         return true;
      }
      
      override public function toString() : String
      {
         return "[UpgradeSlot " + name + "]";
      }
      
      protected function updateSlot() : void
      {
         var slotSize:int = 0;
         var iconScale:Number = NaN;
         if(_data != null)
         {
            if(Boolean(this.btnChange))
            {
               this.btnChange.visible = true;
               this.btnDrop.visible = true;
            }
            content = _data.icon;
            slotSize = 140;
            iconScale = Math.min(slotSize / _content.width,slotSize / _content.height,1);
            contentCanvas.x = (slotSize - _content.width) / 2;
            contentCanvas.y = (slotSize - _content.height) / 2;
            if(Boolean(this.textFieldLabel))
            {
               this.textFieldLabel.text = "";
            }
            if(Boolean(this.textField) && Boolean(content))
            {
               UpgradeWindow.fitText(this.textField,String(this.data.name),this.textField.width - 4);
            }
         }
         else
         {
            if(Boolean(this.border.inner))
            {
               this.border.inner.visible = false;
            }
            if(Boolean(this.btnChange))
            {
               this.btnChange.visible = false;
               this.btnDrop.visible = false;
            }
            content = null;
            if(Boolean(this.textField))
            {
               this.textField.defaultTextFormat = this.textField.getTextFormat();
               this.textField.text = "  ";
            }
            if(Boolean(this.textFieldLabel))
            {
               this.textFieldLabel.text = this._label;
            }
         }
      }
      
      protected function getStatePrefix() : String
      {
         if(this._highlighted)
         {
            return !_data ? this.statesHighlighted : this.statesFilledHighlighted;
         }
         if(_data)
         {
            return this.statesFilled;
         }
         return "";
      }
      
      override protected function setState(state:String) : void
      {
         _state = state;
         var states:Array = _stateMap[state];
         var prefix:String = this.getStatePrefix();
         if(states == null || states.length == 0)
         {
            return;
         }
         var stateCount:uint = states.length;
         var i:uint = 0;
         var frameLabel:String;
         for(; i < stateCount; i++)
         {
            frameLabel = prefix + states[i];
            _newFrame = frameLabel;
            invalidateState();
            return;
         }
      }
      
      override protected function configUI() : void
      {
         super.configUI();
         mouseChildren = true;
         if(this.selectedBorder)
         {
            this.selectedBorder.visible = false;
         }
         if(this.btnDrop)
         {
            this.btnDrop.addEventListener(ButtonEvent.CLICK,this.onDropClicked);
         }
         if(this.border && this.border.inner)
         {
            this.border.inner.alpha = 0.7;
            this.border.inner.visible = false;
         }
      }
      
      private function onDropClicked(event:ButtonEvent) : void
      {
         this.dropSlot();
      }
      
      override protected function draw() : void
      {
         var transitions:Array;
         var i:uint;
         var transition:Object;
         var target:Sprite;
         trace("UT::redraw");
         if(isInvalid(InvalidationType.DATA))
         {
            this.updateSlot();
         }
         if(isInvalid(InvalidationType.STATE))
         {
            mouseChildren = true;
            if(this.btnChange)
            {
               this.btnChange.enabled = true;
            }
            if(this._transitionMap[_newFrame])
            {
               transitions = this._transitionMap[_newFrame];
               i = 0;
               while(i < transitions.length)
               {
                  transition = transitions[i];
                  target = transition.target;
                  if(target)
                  {
                     if(target === this.border)
                     {
                        addEventListener(Event.ENTER_FRAME,this.onBorderResize);
                        transition.options.onComplete = this.stopFrameHandle;
                     }
                     TweenLite.killTweensOf(target);
                     transition.options.ease = Back.easeOut;
                     TweenLite.to(target,0.2,transition.options);
                  }
                  i++;
               }
               _newFrame = null;
            }
         }
      }
      
      private function stopFrameHandle() : void
      {
         removeEventListener(Event.ENTER_FRAME,this.onBorderResize);
      }
   }
}

