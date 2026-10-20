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
      
      public function set data(value:Object) : void
      {
         _data = value;
         invalidateData();
      }
      
      public function get content() : Sprite
      {
         return this._content;
      }
      
      public function set content(value:Sprite) : void
      {
         if(value != this._content)
         {
            if(this._content)
            {
               if(this.contentCanvas.contains(this._content))
               {
                  this.contentCanvas.removeChild(this._content);
               }
            }
            _content = value;
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
      
      public function set descriptor(value:uint) : void
      {
         _descriptor = value;
      }
      
      public function setStage(value:Stage) : void
      {
         if(this._stageRef == null && value != null)
         {
            _stageRef = value;
            DragManager.init(value);
         }
      }
      
      override protected function configUI() : void
      {
         super.configUI();
         var anchor:Sprite = getChildByName("spriteAnchor") as Sprite;
         if(anchor)
         {
            this.contentCanvas.x = anchor.x;
            this.contentCanvas.y = anchor.y;
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
      
      protected function handleMouseOver(e:MouseEvent) : void
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
      
      protected function handleMouseDown(e:MouseEvent) : void
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
      
      protected function handleMouseRollOver(event:MouseEvent) : void
      {
         if(!enabled)
         {
            return;
         }
         this.setState("over");
      }
      
      protected function handleMouseRollOut(event:MouseEvent) : void
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
      
      protected function handleMouseMove(e:MouseEvent) : void
      {
         var dragStartEvent:DragEvent;
         if(mouseX > this._mouseDownX + 3 || mouseX < this._mouseDownX - 3 || mouseY > this._mouseDownY + 3 || mouseY < this._mouseDownY - 3)
         {
            this.cleanupDragListeners();
            dragStartEvent = new DragEvent(DragEvent.DRAG_START,_data,this,null,this._content);
            dispatchEvent(dragStartEvent);
            this.handleDragStartEvent(dragStartEvent);
         }
      }
      
      protected function handleMouseUp(e:MouseEvent) : void
      {
         this.cleanupDragListeners();
         dispatchEvent(new ButtonEvent(ButtonEvent.CLICK));
      }
      
      public function handleDragStartEvent(e:DragEvent) : void
      {
      }
      
      public function handleDropEvent(e:DragEvent) : Boolean
      {
         var acceptDrop:Boolean = true;
         if(acceptDrop)
         {
            content = e.dragSprite;
         }
         return acceptDrop;
      }
      
      public function handleDragEndEvent(e:DragEvent, wasValidDrop:Boolean) : void
      {
         if(wasValidDrop)
         {
            content = null;
         }
         else
         {
            this.contentCanvas.addChild(e.dragSprite);
            e.dragSprite.x = 0;
            e.dragSprite.y = 0;
         }
      }
      
      protected function setState(state:String) : void
      {
         _state = state;
         var states:Array = this._stateMap[state];
         if(states == null || states.length == 0)
         {
            return;
         }
         var sl:uint = states.length;
         var j:uint = 0;
         var thisLabel:String;
         while(j < sl)
         {
            thisLabel = states[j];
            if(_labelHash[thisLabel])
            {
               _newFrame = thisLabel;
               invalidateState();
               return;
            }
            j++;
         }
      }
   }
}

