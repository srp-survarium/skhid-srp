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
         var _loc1_:MovieClip = getChildByName("spriteAnchor") as MovieClip;
         if(_loc1_ && contentCanvas)
         {
            contentCanvas = new Sprite();
            _loc1_.addChild(contentCanvas);
            if(_loc1_.x == -30)
            {
               _loc1_.x -= 9;
               _loc1_.y -= 7;
               this.patternMC.y = 19;
            }
            else
            {
               this.patternMC.y = 33;
            }
            this.patternMC.x = -17;
            this.patternMC.width = this.patternMC.height = 140;
            this.patternMC.mask = this.maskSymbol2;
            _loc1_.mask = this.maskSymbol;
            setChildIndex(this.textField,0);
         }
         this.textField.mouseEnabled = false;
         this.textFieldLabel.mouseEnabled = false;
         _loc1_.y += 13;
      }
      
      public function setTransitionSize(param1:uint, param2:uint) : void
      {
         var _loc3_:int = (param1);
         var _loc4_:int = (param2);
         var _loc5_:Number = 10;
         var _loc6_:Number = _loc4_ - _loc3_ / 2;
         var _loc7_:Number = _loc6_;
         posY = _loc6_;
         var _loc8_:Number = 37;
         var _loc9_:Number = 2;
         _transitionMap = {
            up:[{
               target:this.maskSymbol,
               options:{
                  x:_loc8_,
                  y:_loc6_,
                  width:_loc3_,
                  height:_loc3_
               }
            },{
               target:this.border,
               options:{
                  x:_loc8_,
                  y:_loc6_,
                  width:_loc3_ + _loc9_,
                  height:_loc3_ + _loc9_
               }
            }],
            over:[{
               target:this.maskSymbol,
               options:{
                  x:_loc8_,
                  y:_loc6_,
                  width:_loc3_ + _loc5_,
                  height:_loc3_ + _loc5_
               }
            },{
               target:this.border,
               options:{
                  x:_loc8_,
                  y:_loc6_,
                  width:_loc3_ + _loc5_ + _loc9_,
                  height:_loc3_ + _loc5_ + _loc9_
               }
            }],
            highlighted_up:[{
               target:this.maskSymbol,
               options:{
                  x:_loc8_,
                  y:_loc6_,
                  width:_loc3_ + _loc5_,
                  height:_loc3_ + _loc5_
               }
            },{
               target:this.border,
               options:{
                  x:_loc8_,
                  y:_loc6_,
                  width:_loc3_ + _loc5_ + _loc9_,
                  height:_loc3_ + _loc5_ + _loc9_
               }
            }],
            highlighted_over:[{
               target:this.maskSymbol,
               options:{
                  x:_loc8_,
                  y:_loc7_,
                  width:_loc4_ + _loc5_,
                  height:_loc4_ + _loc5_
               }
            },{
               target:this.border,
               options:{
                  x:_loc8_,
                  y:_loc7_,
                  width:_loc4_ + _loc5_ + _loc9_,
                  height:_loc4_ + _loc5_ + _loc9_
               }
            }],
            filled_up:[{
               target:this.maskSymbol,
               options:{
                  x:_loc8_,
                  y:_loc7_,
                  width:_loc4_,
                  height:_loc4_
               }
            },{
               target:this.border,
               options:{
                  x:_loc8_,
                  y:_loc7_,
                  width:_loc4_ + _loc9_,
                  height:_loc4_ + _loc9_
               }
            }],
            filled_over:[{
               target:this.maskSymbol,
               options:{
                  x:_loc8_,
                  y:_loc7_,
                  width:_loc4_,
                  height:_loc4_
               }
            },{
               target:this.border,
               options:{
                  x:_loc8_,
                  y:_loc7_,
                  width:_loc4_ + _loc9_,
                  height:_loc4_ + _loc9_
               }
            }],
            highlighted_filled_up:[{
               target:this.maskSymbol,
               options:{
                  x:_loc8_,
                  y:_loc7_,
                  width:_loc4_ + _loc5_,
                  height:_loc4_ + _loc5_
               }
            },{
               target:this.border,
               options:{
                  x:_loc8_,
                  y:_loc7_,
                  width:_loc4_ + _loc5_ + _loc9_,
                  height:_loc4_ + _loc5_ + _loc9_
               }
            }],
            highlighted_filled_over:[{
               target:this.maskSymbol,
               options:{
                  x:_loc8_,
                  y:_loc7_,
                  width:_loc4_ + _loc5_,
                  height:_loc4_ + _loc5_
               }
            },{
               target:this.border,
               options:{
                  x:_loc8_,
                  y:_loc7_,
                  width:_loc4_ + _loc5_ + _loc9_,
                  height:_loc4_ + _loc5_ + _loc9_
               }
            }]
         };
      }
      
      private function onBorderResize(param1:Event) : void
      {
         var _loc2_:uint;
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
            _loc2_ = 5;
            this.selectedBorder.width = this.maskSymbol.width + _loc2_ * 2;
            this.selectedBorder.height = this.maskSymbol.height + _loc2_ * 2;
            this.selectedBorder.x = this.maskSymbol.x - this.selectedBorder.width / 2;
            this.selectedBorder.y = this.maskSymbol.y - this.selectedBorder.height / 2;
         }
      }
      
      public function get owner() : UIComponent
      {
         return this._owner;
      }
      
      public function set owner(param1:UIComponent) : void
      {
         _owner = param1;
      }
      
      public function set label(param1:String) : void
      {
         _label = param1;
         invalidateData();
      }
      
      public function get selectable() : Boolean
      {
         return this._selectable;
      }
      
      public function set selectable(param1:Boolean) : void
      {
         _selectable = param1;
      }
      
      public function get highlighted() : Boolean
      {
         return this._highlighted;
      }
      
      public function set highlighted(param1:Boolean) : void
      {
         if(this._highlighted == param1)
         {
            return;
         }
         _highlighted = param1;
         this.setState("up");
         invalidateState();
         invalidateData();
      }
      
      public function get selected() : Boolean
      {
         return this._selected;
      }
      
      public function set selected(param1:Boolean) : void
      {
         _selected = param1;
         this.setState("up");
      }
      
      override public function get data() : Object
      {
         return _data;
      }
      
      override public function set data(param1:Object) : void
      {
         if(_data != param1)
         {
            _data = param1;
            stackTrace("US::setdata");
         }
         this.setState("up");
         invalidateData();
      }
      
      public function get index() : uint
      {
         return this._index;
      }
      
      public function set index(param1:uint) : void
      {
         _index = param1;
         _descriptor = param1;
      }
      
      public function setListData(param1:ListData) : void
      {
         index = param1.index;
         selected = param1.selected;
      }
      
      public function setData(param1:Object) : void
      {
         _lastData = this.data;
         this.data = param1;
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
      
      override public function handleDragStartEvent(param1:DragEvent) : void
      {
         this.setState("over");
         content = null;
         this.textField.text = "  ";
      }
      
      override public function handleDragEndEvent(param1:DragEvent, param2:Boolean) : void
      {
         trace("UT::handleDragEndEvent::",param1.dropTarget);
         if(param1.dropTarget != this)
         {
            this.dropSlot();
            dispatchEvent(param1);
         }
         else
         {
            data = param1.dragData;
            if(this.btnChange)
            {
               this.btnChange.enabled = false;
            }
            mouseChildren = false;
         }
         trace("UT::handleDragEndEvent::end");
      }
      
      override public function handleDropEvent(param1:DragEvent) : Boolean
      {
         if(enabled == false || this.highlighted == false)
         {
            return false;
         }
         dispatchEvent(param1);
         return true;
      }
      
      override public function toString() : String
      {
         return "[UpgradeSlot " + name + "]";
      }
      
      protected function updateSlot() : void
      {
         var _loc1_:int = 0;
         var _loc2_:Number = NaN;
         if(_data != null)
         {
            if(Boolean(this.btnChange))
            {
               this.btnChange.visible = true;
               this.btnDrop.visible = true;
            }
            content = _data.icon;
            _loc1_ = 140;
            _loc2_ = Math.min(_loc1_ / _content.width,_loc1_ / _content.height,1);
            contentCanvas.x = (_loc1_ - _content.width) / 2;
            contentCanvas.y = (_loc1_ - _content.height) / 2;
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
      
      override protected function setState(param1:String) : void
      {
         _state = param1;
         var _loc2_:Array = _stateMap[param1];
         var _loc3_:String = this.getStatePrefix();
         if(_loc2_ == null || _loc2_.length == 0)
         {
            return;
         }
         var _loc4_:uint = _loc2_.length;
         var _loc5_:uint = 0;
         var _loc6_:String;
         for(; _loc5_ < _loc4_; _loc5_++)
         {
            _loc6_ = _loc3_ + _loc2_[_loc5_];
            _newFrame = _loc6_;
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
      
      private function onDropClicked(param1:ButtonEvent) : void
      {
         this.dropSlot();
      }
      
      override protected function draw() : void
      {
         var _loc1_:Array;
         var _loc2_:uint;
         var _loc3_:Object;
         var _loc4_:Sprite;
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
               _loc1_ = this._transitionMap[_newFrame];
               _loc2_ = 0;
               while(_loc2_ < _loc1_.length)
               {
                  _loc3_ = _loc1_[_loc2_];
                  _loc4_ = _loc3_.target;
                  if(_loc4_)
                  {
                     if(_loc4_ === this.border)
                     {
                        addEventListener(Event.ENTER_FRAME,this.onBorderResize);
                        _loc3_.options.onComplete = this.stopFrameHandle;
                     }
                     TweenLite.killTweensOf(_loc4_);
                     _loc3_.options.ease = Back.easeOut;
                     TweenLite.to(_loc4_,0.2,_loc3_.options);
                  }
                  _loc2_++;
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

