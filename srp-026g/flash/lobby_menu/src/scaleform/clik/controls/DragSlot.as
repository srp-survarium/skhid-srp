package scaleform.clik.controls
{
   import flash.display.Sprite;
   import flash.display.Stage;
   import flash.events.MouseEvent;
   import scaleform.clik.constants.InvalidationType;
   import scaleform.clik.events.ButtonEvent;
   import scaleform.clik.events.DragEvent;
   import scaleform.clik.interfaces.IDragSlot;
   import scaleform.clik.managers.DragManager;
   
   public class DragSlot extends ToolTip implements IDragSlot
   {
      
      protected var _mouseDownX:Number;
      
      protected var _mouseDownY:Number;
      
      protected var _content:Sprite;
      
      protected var _stageRef:Stage = null;
      
      protected var _newFrame:String;
      
      protected var _descriptor:uint;
      
      protected var _stateMap:Object = {
         up:["up"],
         over:["over"],
         down:["down"],
         release:["release","over"],
         out:["out","up"],
         disabled:["disabled"],
         selecting:["selecting","over"]
      };
      
      protected var _state:String;
      
      public var dragEnabled:Boolean = true;
      
      public var contentCanvas:Sprite;
      
      public function DragSlot()
      {
         super();
         if(!this.contentCanvas)
         {
            contentCanvas = new Sprite();
            addChild(this.contentCanvas);
         }
         if(stage)
         {
            DragManager.init(stage);
         }
         trackAsMenu = true;
      }
      
      public function get data() : Object
      {
         return _data;
      }
      
      public function set data(param1:Object) : void
      {
         _data = param1;
         invalidateData();
      }
      
      public function get content() : Sprite
      {
         return this._content;
      }
      
      public function set content(param1:Sprite) : void
      {
         if(param1 != this._content)
         {
            if(this._content)
            {
               if(this.contentCanvas.contains(this._content))
               {
                  this.contentCanvas.removeChild(this._content);
               }
            }
            _content = param1;
            if(this._content == null)
            {
               return;
            }
            if(this._content != this)
            {
               this._content.x = 0;
               this._content.y = 0;
               this.contentCanvas.addChild(this._content);
               this._content.mouseChildren = false;
            }
         }
      }
      
      public function get descriptor() : uint
      {
         return this._descriptor;
      }
      
      public function set descriptor(param1:uint) : void
      {
         _descriptor = param1;
      }
      
      public function setStage(param1:Stage) : void
      {
         if(this._stageRef == null && param1 != null)
         {
            _stageRef = param1;
            DragManager.init(param1);
         }
      }
      
      override protected function configUI() : void
      {
         super.configUI();
         var _loc1_:Sprite = getChildByName("spriteAnchor") as Sprite;
         if(_loc1_)
         {
            this.contentCanvas.x = _loc1_.x;
            this.contentCanvas.y = _loc1_.y;
         }
         addEventListener(MouseEvent.MOUSE_OVER,this.handleMouseOver,true,0,true);
         addEventListener(MouseEvent.MOUSE_DOWN,this.handleMouseDown,false,0,true);
         addEventListener(MouseEvent.ROLL_OVER,this.handleMouseRollOver,false,0,true);
         addEventListener(MouseEvent.ROLL_OUT,this.handleMouseRollOut,false,0,true);
      }
      
      override protected function draw() : void
      {
         super.draw();
         if(isInvalid(InvalidationType.STATE))
         {
            if(this._newFrame)
            {
               gotoAndPlay(this._newFrame);
               _newFrame = null;
            }
         }
      }
      
      override public function toString() : String
      {
         return "[CLIK DragSlot " + name + "]";
      }
      
      protected function handleMouseOver(param1:MouseEvent) : void
      {
         if(DragManager.inDrag())
         {
         }
         else
         {
            if(this.content != null)
            {
            }
         }
      }
      
      protected function handleMouseDown(param1:MouseEvent) : void
      {
         if(DragManager.inDrag() || !enabled)
         {
            return;
         }
         if(this._content != null && this.dragEnabled)
         {
            stage.addEventListener(MouseEvent.MOUSE_UP,this.handleMouseUp,false,0,true);
            stage.addEventListener(MouseEvent.MOUSE_MOVE,this.handleMouseMove,false,0,true);
            _mouseDownX = mouseX;
            _mouseDownY = mouseY;
         }
      }
      
      protected function handleMouseRollOver(param1:MouseEvent) : void
      {
         if(!enabled)
         {
            return;
         }
         this.setState("over");
      }
      
      protected function handleMouseRollOut(param1:MouseEvent) : void
      {
         if(!enabled)
         {
            return;
         }
         this.setState("out");
      }
      
      protected function cleanupDragListeners() : void
      {
         stage.removeEventListener(MouseEvent.MOUSE_UP,this.handleMouseUp,false);
         stage.removeEventListener(MouseEvent.MOUSE_MOVE,this.handleMouseMove,false);
         _mouseDownX = undefined;
         _mouseDownY = undefined;
      }
      
      protected function handleMouseMove(param1:MouseEvent) : void
      {
         var _loc2_:DragEvent;
         if(mouseX > this._mouseDownX + 3 || mouseX < this._mouseDownX - 3 || mouseY > this._mouseDownY + 3 || mouseY < this._mouseDownY - 3)
         {
            this.cleanupDragListeners();
            _loc2_ = new DragEvent(DragEvent.DRAG_START,_data,this,null,this._content);
            dispatchEvent(_loc2_);
            this.handleDragStartEvent(_loc2_);
         }
      }
      
      protected function handleMouseUp(param1:MouseEvent) : void
      {
         this.cleanupDragListeners();
         dispatchEvent(new ButtonEvent(ButtonEvent.CLICK));
      }
      
      public function handleDragStartEvent(param1:DragEvent) : void
      {
      }
      
      public function handleDropEvent(param1:DragEvent) : Boolean
      {
         var _loc2_:Boolean = true;
         if(_loc2_)
         {
            content = param1.dragSprite;
         }
         return _loc2_;
      }
      
      public function handleDragEndEvent(param1:DragEvent, param2:Boolean) : void
      {
         if(param2)
         {
            content = null;
         }
         else
         {
            this.contentCanvas.addChild(param1.dragSprite);
            param1.dragSprite.x = 0;
            param1.dragSprite.y = 0;
         }
      }
      
      protected function setState(param1:String) : void
      {
         _state = param1;
         var _loc2_:Array = this._stateMap[param1];
         if(_loc2_ == null || _loc2_.length == 0)
         {
            return;
         }
         var _loc3_:uint = _loc2_.length;
         var _loc4_:uint = 0;
         var _loc5_:String;
         while(_loc4_ < _loc3_)
         {
            _loc5_ = _loc2_[_loc4_];
            if(_labelHash[_loc5_])
            {
               _newFrame = _loc5_;
               invalidateState();
               return;
            }
            _loc4_++;
         }
      }
   }
}

